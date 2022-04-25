use crate::audio_cache::AudioCache;
use crate::error::{HomeSpeakError, Result};
use azure_tts::VoiceSegment;
use log::*;
use secrecy::{ExposeSecret, Secret};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Cursor;
use std::io::{Read, Seek};
use tokio::task;

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
const AZURE_FORMAT_VERSION: u32 = 3;

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
    hasher.update(&[style as u8]);
    hasher.update(AZURE_FORMAT_VERSION.to_be_bytes());
    // Turning it into json to hash is a hack.
    // TODO: hash the type not the json
    hasher.update(serde_json::to_string(&voice.gender).unwrap());
    let hashed = hasher.finalize();
    format!("{}-{:x}", voice.name, hashed)
}

#[derive(Deserialize, Debug, Clone, Copy)]
pub enum TtsService {
    Azure,
    Google,
}

// These are styles that apply to en-US-SaraNeural
// since that's the most used voice
#[derive(Debug, Clone, Copy)]
pub enum AzureVoiceStyle {
    Plain,
    Angry,
    Cheerful,
    Sad,
}

pub struct SpeechService {
    google_speech_client: google_tts::GoogleTtsClient,
    azure_speech_client: azure_tts::VoiceService,
    audio_cache: Option<AudioCache>,
    google_voice: google_tts::VoiceProps,
    azure_voice: azure_tts::VoiceSettings,
    azure_audio_format: azure_tts::AudioFormat,
}

pub trait PlayAble: std::io::Read + std::io::Seek + Send + Sync {}

impl PlayAble for Cursor<Vec<u8>> {}
impl PlayAble for File {}

impl SpeechService {
    pub fn new(
        google_api_key: Secret<String>,
        azure_subscription_key: Secret<String>,
        cache_dir_path: Option<String>,
    ) -> Result<SpeechService> {
        let google_speech_client =
            google_tts::GoogleTtsClient::new(google_api_key.expose_secret().to_owned());
        let azure_speech_client = azure_tts::VoiceService::new(
            azure_subscription_key.expose_secret(),
            azure_tts::Region::uksouth,
        );

        let audio_cache = match cache_dir_path {
            Some(path) => Some(AudioCache::new(path)?),
            None => None,
        };

        Ok(SpeechService {
            google_speech_client,
            azure_speech_client,
            audio_cache,
            google_voice: google_tts::VoiceProps::default_english_female_wavenet(),
            azure_voice: azure_tts::EnUsVoices::SaraNeural.to_voice_settings(),
            azure_audio_format: azure_tts::AudioFormat::Audio48khz192kbitrateMonoMp3,
        })
    }

    async fn play<R: Read + Seek + Send + Sync + 'static>(&self, data: R) -> Result<()> {
        // Simple way to spawn a new sink for every new sample
        task::spawn_blocking(move || -> Result<()> {
            let (_stream, stream_handle) = rodio::OutputStream::try_default()
                .map_err(|_| HomeSpeakError::FailedToCreateAnOutputStream)?;
            let sink = rodio::Sink::try_new(&stream_handle)
                .map_err(|_| HomeSpeakError::FailedToCreateASink)?;
            sink.append(
                rodio::Decoder::new(data).map_err(|_| HomeSpeakError::FailedToDecodeAudioFile)?,
            );
            // TODO(David): Here we could pause/resume playing audio
            sink.sleep_until_end();
            Ok(())
        })
        .await
        .expect("Tokio blocking task for rodio failed")?;
        Ok(())
    }

    async fn say_google(&self, text: &str) -> Result<()> {
        let playable: Box<dyn PlayAble> = if let Some(audio_cache) = &self.audio_cache {
            let file_key = hash_google_tts(text, &self.google_voice);
            if let Some(file) = audio_cache.get(&file_key) {
                info!("Using cached value with key {}", file_key);
                file
            } else {
                info!("Writing new file with key {}", file_key);
                let data = self
                    .google_speech_client
                    .synthesize(
                        google_tts::TextInput::with_text(text.to_owned()),
                        self.google_voice.clone(),
                        google_tts::AudioConfig::default_with_encoding(
                            google_tts::AudioEncoding::Mp3,
                        ),
                    )
                    .await
                    .map_err(|_| HomeSpeakError::GoogleTtsError)?;
                audio_cache.set(
                    &file_key,
                    data.as_byte_stream()
                        .map_err(|_| HomeSpeakError::GoogleTtsError)?,
                )?;
                Box::new(Cursor::new(
                    data.as_byte_stream()
                        .map_err(|_| HomeSpeakError::GoogleTtsError)?,
                ))
            }
        } else {
            let data = self
                .google_speech_client
                .synthesize(
                    google_tts::TextInput::with_text(text.to_owned()),
                    self.google_voice.clone(),
                    google_tts::AudioConfig::default_with_encoding(google_tts::AudioEncoding::Mp3),
                )
                .await
                .map_err(|_| HomeSpeakError::GoogleTtsError)?;

            Box::new(Cursor::new(
                data.as_byte_stream()
                    .map_err(|_| HomeSpeakError::GoogleTtsError)?,
            ))
        };
        self.play(playable).await?;
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
            AzureVoiceStyle::Plain => VoiceSegment::plain(text),
            AzureVoiceStyle::Angry => VoiceSegment::with_expression(text, azure_tts::Style::Angry),
            AzureVoiceStyle::Sad => VoiceSegment::with_expression(text, azure_tts::Style::Sad),
            AzureVoiceStyle::Cheerful => {
                VoiceSegment::with_expression(text, azure_tts::Style::Cheerful)
            }
        };
        segments.push(contents);

        let sound: Box<dyn PlayAble> = if let Some(ref audio_cache) = self.audio_cache {
            let file_key = hash_azure_tts(text, voice, self.azure_audio_format, style);
            if let Some(file) = audio_cache.get(&file_key) {
                info!("Using cached value with key {}", file_key);
                file
            } else {
                info!("Writing new file with key {}", file_key);
                let data = self
                    .azure_speech_client
                    .synthesize_segments(segments, voice, self.azure_audio_format)
                    .await?;
                audio_cache.set(&file_key, data.clone())?;
                Box::new(Cursor::new(data))
            }
        } else {
            let data = self
                .azure_speech_client
                .synthesize_segments(segments, voice, self.azure_audio_format)
                .await?;
            Box::new(Cursor::new(data))
        };
        self.play(sound).await?;
        Ok(())
    }

    pub async fn say_azure(&mut self, text: &str) -> Result<()> {
        // This cloning here is lame...
        self.say_azure_with_voice(text, &self.azure_voice.clone(), AzureVoiceStyle::Plain)
            .await
    }

    pub async fn say_azure_with_feelings(
        &mut self,
        text: &str,
        style: AzureVoiceStyle,
    ) -> Result<()> {
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
