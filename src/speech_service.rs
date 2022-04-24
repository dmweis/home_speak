use crate::audio_cache::AudioCache;
use crate::error::{HomeSpeakError, Result};
use log::*;
use secrecy::{ExposeSecret, Secret};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::io::Cursor;
use std::io::{Read, Seek};

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

fn hash_azure_tts(
    text: &str,
    voice: &azure_tts::VoiceSettings,
    format: azure_tts::AudioFormat,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text);
    hasher.update(&voice.name);
    hasher.update(&voice.language);
    hasher.update(format.as_string());
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

pub struct SpeechService {
    google_speech_client: google_tts::GoogleTtsClient,
    azure_speech_client: azure_tts::VoiceService,
    audio_cache: Option<AudioCache>,
    google_voice: google_tts::VoiceProps,
    azure_voice: azure_tts::VoiceSettings,
    azure_audio_format: azure_tts::AudioFormat,
}

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

    fn play<R: Read + Seek + Send + 'static>(&self, data: R) -> Result<()> {
        // Simple way to spawn a new sink for every new sample
        let (_stream, stream_handle) = rodio::OutputStream::try_default().unwrap();
        let sink = rodio::Sink::try_new(&stream_handle).unwrap();
        sink.append(
            rodio::Decoder::new(data).map_err(|_| HomeSpeakError::FailedToDecodeAudioFile)?,
        );
        sink.sleep_until_end();
        Ok(())
    }

    async fn say_google(&self, text: &str) -> Result<()> {
        if let Some(audio_cache) = &self.audio_cache {
            let file_key = hash_google_tts(text, &self.google_voice);
            if let Some(file) = audio_cache.get(&file_key) {
                info!("Using cached value");
                self.play(file)?;
            } else {
                info!("Writing new file");
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
                let buffer = Cursor::new(
                    data.as_byte_stream()
                        .map_err(|_| HomeSpeakError::GoogleTtsError)?,
                );
                self.play(buffer)?;
            }
            Ok(())
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

            let buffer = Cursor::new(
                data.as_byte_stream()
                    .map_err(|_| HomeSpeakError::GoogleTtsError)?,
            );
            self.play(buffer)?;
            Ok(())
        }
    }

    async fn say_azure(&mut self, text: &str) -> Result<()> {
        if let Some(ref audio_cache) = self.audio_cache {
            let file_key = hash_azure_tts(text, &self.azure_voice, self.azure_audio_format);
            if let Some(file) = audio_cache.get(&file_key) {
                info!("Using cached value");
                self.play(file)?;
            } else {
                info!("Writing new file");
                let data = self
                    .azure_speech_client
                    .synthesize(text, &self.azure_voice, self.azure_audio_format)
                    .await?;
                audio_cache.set(&file_key, data.clone())?;
                self.play(Cursor::new(data))?;
            }
        } else {
            let data = self
                .azure_speech_client
                .synthesize(text, &self.azure_voice, self.azure_audio_format)
                .await?;
            self.play(Cursor::new(data))?;
        }
        Ok(())
    }

    pub async fn say(&mut self, text: &str, service: TtsService) -> Result<()> {
        match service {
            TtsService::Azure => self.say_azure(text).await?,
            TtsService::Google => self.say_google(text).await?,
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn sample_azure_languages(&mut self, text: &str) -> Result<()> {
        let languages = self.azure_speech_client.list_voices().await?;
        for language in languages {
            if language.locale == "en-US" {
                println!(
                    "Lang name {} locale {}",
                    language.short_name, language.locale
                );
                let voice_settings = language.to_voice_settings();
                let data = self
                    .azure_speech_client
                    .synthesize(
                        text,
                        &voice_settings,
                        azure_tts::AudioFormat::Audio48khz192kbitrateMonoMp3,
                    )
                    .await?;
                let file_key = hash_azure_tts(text, &voice_settings, self.azure_audio_format);
                if let Some(audio_cache) = &self.audio_cache {
                    audio_cache.set(&file_key, data.clone())?;
                }
                self.play(Cursor::new(data))?;
            }
        }
        Ok(())
    }
}
