mod audio_player;

use crate::audio_cache::AudioCache;
use crate::error::HomeSpeakError;
use crate::{eleven_labs_client, AUDIO_FILE_EXTENSION};
use anyhow::Context;
use audio_player::AudioPlayerCommand;
use base64::{engine::general_purpose, Engine as _};
use log::*;
use secrecy::{ExposeSecret, Secret};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{io::Cursor, sync::mpsc::Sender};
use tokio::sync::mpsc::UnboundedSender as TokioSender;

use anyhow::Result;

use self::audio_player::create_player;
pub use self::audio_player::Playable;

fn hash_google_tts(text: &str, voice: &google_tts::VoiceProps) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text);
    // Turning it into json to hash is a hack.
    // TODO: hash the type not the json
    hasher.update(serde_json::to_string(voice).unwrap());
    let hashed = hasher.finalize();
    format!(
        "{}-{:x}",
        voice
            .name
            .to_owned()
            .unwrap_or_else(|| String::from("Unknown")),
        hashed
    )
}

// Used to invalidate old cache
const AZURE_FORMAT_VERSION: u32 = 4;

fn hash_azure_tts(
    text: &str,
    voice: &azure_tts::VoiceSettings,
    format: azure_tts::AudioFormat,
    style: AzureVoiceStyle,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text);
    hasher.update(&voice.name);
    hasher.update(&voice.language);
    hasher.update(format.as_string());
    hasher.update([style as u8]);
    hasher.update(AZURE_FORMAT_VERSION.to_be_bytes());
    // Turning it into json to hash is a hack.
    // TODO: hash the type not the json
    hasher.update(serde_json::to_string(&voice.gender).unwrap());
    let hashed = hasher.finalize();
    format!("{}-{:x}", voice.name, hashed)
}

fn hash_eleven_labs_tts(text: &str, voice_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text);
    hasher.update(voice_id);
    hasher.update(AZURE_FORMAT_VERSION.to_be_bytes());
    let hashed = hasher.finalize();
    format!("eleven-{:x}", hashed)
}

#[derive(Deserialize, Debug, Clone, Copy)]
pub enum TtsService {
    Azure,
    Google,
}

// These are styles that apply to en-US-SaraNeural
// since that's the most used voice
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, Default)]
pub enum AzureVoiceStyle {
    #[default]
    Plain,
    Angry,
    Cheerful,
    Sad,
}

pub struct SpeechService {
    google_speech_client: google_tts::GoogleTtsClient,
    azure_speech_client: azure_tts::VoiceService,
    audio_cache: AudioCache,
    google_voice: google_tts::VoiceProps,
    azure_voice: azure_tts::VoiceSettings,
    azure_audio_format: azure_tts::AudioFormat,
    audio_service: AudioService,
}

impl SpeechService {
    pub fn new(
        google_api_key: Secret<String>,
        azure_subscription_key: Secret<String>,
        audio_cache: AudioCache,
        audio_service: AudioService,
    ) -> Result<SpeechService> {
        Self::new_with_mqtt(
            google_api_key,
            azure_subscription_key,
            audio_cache,
            audio_service,
        )
    }

    pub fn new_with_mqtt(
        google_api_key: Secret<String>,
        azure_subscription_key: Secret<String>,
        audio_cache: AudioCache,
        audio_service: AudioService,
    ) -> Result<SpeechService> {
        let google_speech_client =
            google_tts::GoogleTtsClient::new(google_api_key.expose_secret().to_owned());
        let azure_speech_client = azure_tts::VoiceService::new(
            azure_subscription_key.expose_secret(),
            azure_tts::Region::uksouth,
        );

        Ok(SpeechService {
            google_speech_client,
            azure_speech_client,
            audio_cache,
            google_voice: google_tts::VoiceProps::default_english_female_wavenet(),
            azure_voice: azure_tts::EnUsVoices::SaraNeural.to_voice_settings(),
            azure_audio_format: azure_tts::AudioFormat::Audio48khz192kbitrateMonoMp3,
            audio_service,
        })
    }

    async fn say_google(&mut self, text: &str) -> Result<()> {
        let file_key = hash_google_tts(text, &self.google_voice);
        let playable: Box<dyn Playable> = if let Some(file) = self.audio_cache.get(&file_key) {
            info!("Using cached value with key {}", file_key);
            file
        } else {
            info!("Writing new file with key {}", file_key);
            let data = self
                .google_speech_client
                .synthesize(
                    google_tts::TextInput::with_text(text.to_owned()),
                    self.google_voice.clone(),
                    google_tts::AudioConfig::default_with_encoding(google_tts::AudioEncoding::Mp3),
                )
                .await
                .map_err(|_| HomeSpeakError::GoogleTtsError)?;
            self.audio_cache.set(
                &file_key,
                data.as_byte_stream()
                    .map_err(|_| HomeSpeakError::GoogleTtsError)?,
            )?;
            Box::new(Cursor::new(
                data.as_byte_stream()
                    .map_err(|_| HomeSpeakError::GoogleTtsError)?,
            ))
        };

        self.audio_service.play(playable)?;
        Ok(())
    }

    async fn say_azure_with_voice(
        &mut self,
        text: &str,
        voice: &azure_tts::VoiceSettings,
        style: AzureVoiceStyle,
    ) -> Result<()> {
        info!("Using {:?} style", &style);
        let mut segments = vec![
            azure_tts::VoiceSegment::silence(
                azure_tts::SilenceAttributeType::Sentenceboundary,
                "50ms".to_owned(),
            ),
            azure_tts::VoiceSegment::silence(
                azure_tts::SilenceAttributeType::Tailing,
                "25ms".to_owned(),
            ),
            azure_tts::VoiceSegment::silence(
                azure_tts::SilenceAttributeType::Leading,
                "25ms".to_owned(),
            ),
        ];
        let contents = match style {
            AzureVoiceStyle::Plain => azure_tts::VoiceSegment::plain(text),
            AzureVoiceStyle::Angry => {
                azure_tts::VoiceSegment::with_expression(text, azure_tts::Style::Angry)
            }
            AzureVoiceStyle::Sad => {
                azure_tts::VoiceSegment::with_expression(text, azure_tts::Style::Sad)
            }
            AzureVoiceStyle::Cheerful => {
                azure_tts::VoiceSegment::with_expression(text, azure_tts::Style::Cheerful)
            }
        };
        segments.push(contents);

        let file_key = hash_azure_tts(text, voice, self.azure_audio_format, style);
        let sound: Box<dyn Playable> = if let Some(file) = self.audio_cache.get(&file_key) {
            info!("Using cached value with key {}", file_key);
            file
        } else {
            info!("Writing new file with key {}", file_key);
            let data = self
                .azure_speech_client
                .synthesize_segments(segments, voice, self.azure_audio_format)
                .await?;
            self.audio_cache.set(&file_key, data.clone())?;
            Box::new(Cursor::new(data))
        };

        self.audio_service.play(sound)?;
        Ok(())
    }

    pub async fn say_azure(&mut self, text: &str) -> Result<()> {
        // This cloning here is lame...
        self.say_azure_with_voice(text, &self.azure_voice.clone(), AzureVoiceStyle::Plain)
            .await
    }

    pub async fn say_azure_with_style(&mut self, text: &str, style: AzureVoiceStyle) -> Result<()> {
        // This cloning here is lame...
        self.say_azure_with_voice(text, &self.azure_voice.clone(), style)
            .await
    }

    pub async fn say(&mut self, text: &str, service: TtsService) -> Result<()> {
        match service {
            TtsService::Azure => self.say_azure(text).await?,
            TtsService::Google => self.say_google(text).await?,
        }
        Ok(())
    }

    pub async fn sample_azure_languages(&mut self, text: &str) -> Result<()> {
        let languages = self.azure_speech_client.list_voices().await?;
        for language in languages {
            if language.locale == "en-US" {
                info!(
                    "Lang name {} locale {}",
                    language.short_name, language.locale
                );
                let message = format!("Hey, my name is {} and {}", language.display_name, text);
                let voice_settings = language.to_voice_settings();
                self.say_azure_with_voice(&message, &voice_settings, AzureVoiceStyle::Plain)
                    .await?;
            }
        }
        Ok(())
    }
}

/// voice Freya
const DEFAULT_ELEVEN_LABS_VOICE_ID: &str = "jsCqWAovK2LkecY7zXl4";

#[derive(Debug, Clone)]
pub struct ElevenSpeechService {
    eleven_labs_client: eleven_labs_client::ElevenLabsTtsClient,
    voice_name_to_voice_id_table: std::collections::HashMap<String, String>,
    audio_cache: AudioCache,
    eleven_labs_default_voice_id: String,
    audio_service: AudioService,
}

impl ElevenSpeechService {
    pub async fn new(
        eleven_labs_api_key: Secret<String>,
        audio_cache: AudioCache,
        audio_service: AudioService,
    ) -> Result<Self> {
        let eleven_labs_client = eleven_labs_client::ElevenLabsTtsClient::new(
            eleven_labs_api_key.expose_secret().to_owned(),
        );

        let voices = eleven_labs_client.voices().await?;
        let voice_name_to_voice_id_table = voices.name_to_id_table();

        info!("voices: {:?}", voice_name_to_voice_id_table);

        Ok(ElevenSpeechService {
            eleven_labs_client,
            voice_name_to_voice_id_table,
            audio_cache,
            eleven_labs_default_voice_id: DEFAULT_ELEVEN_LABS_VOICE_ID.to_owned(),
            audio_service,
        })
    }

    pub async fn say_eleven_with_default_voice(&self, text: &str) -> Result<()> {
        self.say_eleven_with_voice_id(text, &self.eleven_labs_default_voice_id.clone())
            .await?;
        Ok(())
    }

    pub async fn say_eleven(&self, text: &str, voice_name: &str) -> Result<()> {
        let voice_id = self
            .voice_name_to_voice_id_table
            .get(voice_name)
            .context("Unknown voice")?
            .clone();
        info!("Using voice id {} for voice {}", voice_id, voice_name);
        self.say_eleven_with_voice_id(text, &voice_id).await?;
        Ok(())
    }

    pub async fn say_eleven_with_voice_id(&self, text: &str, voice_id: &str) -> Result<()> {
        let file_key = hash_eleven_labs_tts(text, voice_id);
        let sound: Box<dyn Playable> = if let Some(file) = self.audio_cache.get(&file_key) {
            info!("Using cached value with key {}", file_key);
            file
        } else {
            info!("Writing new file with key {}", file_key);
            let data = self.eleven_labs_client.tts(text, voice_id).await?;
            let sound: Box<dyn Playable> = Box::new(Cursor::new(data.to_vec()));
            self.audio_cache.set(&file_key, data.to_vec())?;
            sound
        };
        self.audio_service.play(sound)?;
        Ok(())
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct AudioMessage {
    pub data: String,
    pub format: String,
}

#[derive(Debug, Clone)]
pub struct AudioService {
    audio_sender: Sender<AudioPlayerCommand>,
    audio_data_broadcaster: Option<TokioSender<AudioMessage>>,
}

impl AudioService {
    pub fn new(audio_data_broadcaster: Option<TokioSender<AudioMessage>>) -> Result<Self> {
        let audio_sender = create_player();

        Ok(AudioService {
            audio_sender,
            audio_data_broadcaster,
        })
    }

    pub fn play(&self, mut data: Box<dyn Playable>) -> Result<()> {
        self.publish_audio_file(&mut data)?;
        self.audio_sender
            .send(AudioPlayerCommand::Play(data))
            .unwrap();
        Ok(())
    }

    fn publish_audio_file(&self, data: &mut Box<dyn Playable>) -> Result<()> {
        if let Some(sender) = self.audio_data_broadcaster.as_ref().cloned() {
            let payload = data.as_bytes()?;
            let base64_wav_file: String = general_purpose::STANDARD.encode(payload);
            let message = AudioMessage {
                data: base64_wav_file,
                format: AUDIO_FILE_EXTENSION.to_owned(),
            };
            sender
                .send(message)
                .map_err(|_| HomeSpeakError::AudioChannelSendError)?;
        }
        Ok(())
    }

    pub fn pause(&self) {
        self.audio_sender.send(AudioPlayerCommand::Pause).unwrap();
    }

    pub fn resume(&self) {
        self.audio_sender.send(AudioPlayerCommand::Resume).unwrap();
    }

    pub fn stop(&self) {
        self.audio_sender.send(AudioPlayerCommand::Stop).unwrap();
    }

    pub fn volume(&self, volume: f32) {
        self.audio_sender
            .send(AudioPlayerCommand::Volume(volume))
            .unwrap();
    }
}
