use crate::audio_cache::AudioCache;
use crate::error::{HomeSpeakError, Result};
use log::*;
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
    format!("{:x}", hashed)
}

pub struct SpeechService {
    speech_client: google_tts::GoogleTtsClient,
    audio_cache: Option<AudioCache>,
    voice: google_tts::VoiceProps,
}

impl SpeechService {
    pub fn new(google_api_key: String, cache_dir_path: Option<String>) -> Result<SpeechService> {
        let client = google_tts::GoogleTtsClient::new(google_api_key);

        let audio_cache = match cache_dir_path {
            Some(path) => Some(AudioCache::new(path)?),
            None => None,
        };

        Ok(SpeechService {
            speech_client: client,
            audio_cache,
            voice: google_tts::VoiceProps::default_english_female_wavenet(),
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
            let file_key = hash_google_tts(text, &self.voice);
            if let Some(file) = audio_cache.get(&file_key) {
                info!("Using cached value");
                self.play(file)?;
            } else {
                info!("Writing new file");
                let data = self
                    .speech_client
                    .synthesize(
                        google_tts::TextInput::with_text(text.to_owned()),
                        self.voice.clone(),
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
                .speech_client
                .synthesize(
                    google_tts::TextInput::with_text(text.to_owned()),
                    self.voice.clone(),
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

    pub(crate) async fn say(&mut self, text: &str) -> Result<()> {
        self.say_google(text).await?;
        Ok(())
    }
}
