use crate::audio_cache::AudioCache;
use crate::error::{HomeSpeakError, Result};
use crate::AUDIO_FILE_EXTENSION;
use base64::{engine::general_purpose, Engine as _};
use log::*;
use secrecy::{ExposeSecret, Secret};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Seek;
use std::sync::mpsc::Receiver;
use std::{
    fs::File,
    io::{Cursor, Read},
    sync::mpsc::{channel, Sender},
    thread,
};
use tokio::sync::mpsc::UnboundedSender as TokioSender;

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
    hasher.update([style as u8]);
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
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, Default)]
pub enum AzureVoiceStyle {
    #[default]
    Plain,
    Angry,
    Cheerful,
    Sad,
}

enum AudioPlayerCommand {
    Play(Box<dyn PlayAble>),
    Pause,
    Resume,
    Stop,
    Volume(f32),
}

fn audio_player_loop(
    receiver: &Receiver<AudioPlayerCommand>,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let (_output_stream, output_stream_handle) = rodio::OutputStream::try_default()
        .map_err(|_| HomeSpeakError::FailedToCreateAnOutputStream)?;
    let sink = rodio::Sink::try_new(&output_stream_handle)
        .map_err(|_| HomeSpeakError::FailedToCreateASink)?;
    loop {
        let command = receiver.recv()?;
        match command {
            AudioPlayerCommand::Play(sound) => {
                sink.append(
                    rodio::Decoder::new(sound)
                        .map_err(|_| HomeSpeakError::FailedToDecodeAudioFile)?,
                );
            }
            AudioPlayerCommand::Pause => {
                info!("Pausing audio");
                sink.pause()
            }
            AudioPlayerCommand::Resume => {
                info!("Resuming audio");
                sink.play()
            }
            AudioPlayerCommand::Stop => {
                info!("Stopping audio");
                warn!("Ignoring stop because it destroys the sink");
                // sink.stop()
            }
            AudioPlayerCommand::Volume(volume) => {
                info!("Settings volume to {}", volume);
                sink.set_volume(volume)
            }
        }
    }
}

fn create_player() -> Sender<AudioPlayerCommand> {
    let (sender, receiver) = channel();
    thread::spawn(move || loop {
        // This may miss on sender being dead. But if sender is dead we have bigger issues
        if let Err(e) = audio_player_loop(&receiver) {
            error!("Audio player loop failed with {}", e);
        }
    });
    sender
}

pub struct SpeechService {
    google_speech_client: google_tts::GoogleTtsClient,
    azure_speech_client: azure_tts::VoiceService,
    audio_cache: Option<AudioCache>,
    google_voice: google_tts::VoiceProps,
    azure_voice: azure_tts::VoiceSettings,
    azure_audio_format: azure_tts::AudioFormat,
    audio_sender: Sender<AudioPlayerCommand>,
    audio_data_broadcaster: Option<TokioSender<AudioMessage>>,
}

pub trait PlayAble: std::io::Read + std::io::Seek + Send + Sync {
    fn as_bytes(&mut self) -> Result<Vec<u8>>;
}

impl PlayAble for Cursor<Vec<u8>> {
    fn as_bytes(&mut self) -> Result<Vec<u8>> {
        Ok(self.get_ref().clone())
    }
}
impl PlayAble for File {
    fn as_bytes(&mut self) -> Result<Vec<u8>> {
        let mut buffer = vec![];
        self.read_to_end(&mut buffer)?;
        self.seek(std::io::SeekFrom::Start(0))?;
        Ok(buffer)
    }
}

impl SpeechService {
    pub fn new(
        google_api_key: Secret<String>,
        azure_subscription_key: Secret<String>,
        cache_dir_path: Option<String>,
    ) -> Result<SpeechService> {
        Self::new_with_mqtt(google_api_key, azure_subscription_key, cache_dir_path, None)
    }

    pub fn new_with_mqtt(
        google_api_key: Secret<String>,
        azure_subscription_key: Secret<String>,
        cache_dir_path: Option<String>,
        audio_data_broadcaster: Option<TokioSender<AudioMessage>>,
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

        let audio_sender = create_player();

        Ok(SpeechService {
            google_speech_client,
            azure_speech_client,
            audio_cache,
            google_voice: google_tts::VoiceProps::default_english_female_wavenet(),
            azure_voice: azure_tts::EnUsVoices::SaraNeural.to_voice_settings(),
            azure_audio_format: azure_tts::AudioFormat::Audio48khz192kbitrateMonoMp3,
            audio_sender,
            audio_data_broadcaster,
        })
    }

    async fn play(&mut self, mut data: Box<dyn PlayAble>) -> Result<()> {
        self.publish_audio_file(&mut data)?;
        self.audio_sender
            .send(AudioPlayerCommand::Play(data))
            .unwrap();
        Ok(())
    }

    fn publish_audio_file(&self, data: &mut Box<dyn PlayAble>) -> Result<()> {
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

    async fn say_google(&mut self, text: &str) -> Result<()> {
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

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct AudioMessage {
    pub data: String,
    pub format: String,
}
