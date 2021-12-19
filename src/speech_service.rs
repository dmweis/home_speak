use log::*;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::prelude::*;
use std::io::Cursor;
use std::io::{Read, Seek};
use std::path::Path;

struct AudioCache {
    cache_dir_path: String,
}

fn crate_file_name(text: &str, voice: &google_tts::VoiceProps) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text);
    // Turning it into json to hash is a hack.
    // TODO: hash the type not the json
    hasher.update(serde_json::to_string(voice).unwrap());
    let hashed = hasher.finalize();
    format!("{:x}.mp3", hashed)
}

impl AudioCache {
    fn new(cache_dir_path: String) -> Result<AudioCache, Box<dyn std::error::Error>> {
        let path = Path::new(&cache_dir_path);
        fs::create_dir_all(path)?;
        if !path.exists() {
            return Err("Cache dir path doesn't exist".into());
        }
        Ok(AudioCache { cache_dir_path })
    }

    fn get(&self, text: &str, voice: &google_tts::VoiceProps) -> Option<Box<impl Read + Seek>> {
        let path = Path::new(&self.cache_dir_path);
        let hashed = crate_file_name(text, voice);
        let file_path = path.join(hashed);
        if let Ok(file) = File::open(file_path) {
            Some(Box::new(file))
        } else {
            None
        }
    }

    fn set(
        &self,
        text: &str,
        voice: &google_tts::VoiceProps,
        contents: Vec<u8>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = Path::new(&self.cache_dir_path);
        let hashed = crate_file_name(text, voice);
        let file_path = path.join(hashed);
        let mut file = File::create(file_path)?;
        file.write_all(&contents)?;
        file.flush()?;
        Ok(())
    }
}

pub struct SpeechService {
    speech_client: google_tts::GoogleTtsClient,
    audio_cache: Option<AudioCache>,
    voice: google_tts::VoiceProps,
}

impl SpeechService {
    pub fn new(
        google_api_key: String,
        cache_dir_path: Option<String>,
    ) -> Result<SpeechService, Box<dyn std::error::Error>> {
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

    fn play<R: Read + Seek + Send + 'static>(
        &self,
        data: R,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Simple way to spawn a new sink for every new sample
        let (_stream, stream_handle) = rodio::OutputStream::try_default().unwrap();
        let sink = rodio::Sink::try_new(&stream_handle).unwrap();
        sink.append(rodio::Decoder::new(data)?);
        sink.sleep_until_end();
        Ok(())
    }

    pub async fn say(&self, text: String) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(audio_cache) = &self.audio_cache {
            if let Some(file) = audio_cache.get(&text, &self.voice) {
                info!("Using cached value");
                self.play(file)?;
            } else {
                info!("Writing new file");
                let data = self
                    .speech_client
                    .synthesize(
                        google_tts::TextInput::with_text(text.clone()),
                        self.voice.clone(),
                        google_tts::AudioConfig::default_with_encoding(
                            google_tts::AudioEncoding::Mp3,
                        ),
                    )
                    .await?;
                audio_cache.set(&text, &self.voice, data.as_byte_stream()?)?;
                let buffer = Cursor::new(data.as_byte_stream()?);
                self.play(buffer)?;
            }
            Ok(())
        } else {
            let data = self
                .speech_client
                .synthesize(
                    google_tts::TextInput::with_text(text),
                    self.voice.clone(),
                    google_tts::AudioConfig::default_with_encoding(google_tts::AudioEncoding::Mp3),
                )
                .await?;

            let buffer = Cursor::new(data.as_byte_stream()?);
            self.play(buffer)?;
            Ok(())
        }
    }
}
