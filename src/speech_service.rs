use google_tts;
use std::io::Cursor;
use std::io::BufReader;
use rodio::{self, DeviceTrait};
use log::*;
use std::path::Path;
use std::io::prelude::*;
use sha2::{Sha256, Digest};
use std::fs::File;
use std::io::{ Seek, Read };

struct AudioCache {
    cache_dir_path: String
}


impl AudioCache {
    fn new(cache_dir_path: String) -> Result<AudioCache, Box<dyn std::error::Error>> {
        let path = Path::new(&cache_dir_path);
        if !path.exists() {
            Err("Cache dir path doesn't exist")?;
        }
        Ok(AudioCache{
            cache_dir_path: cache_dir_path,
        })
    }

    fn get(&self, text: String) -> Option<Box<impl Read + Seek>> {
        let path = Path::new(&self.cache_dir_path);
        let mut hasher = Sha256::new();
        hasher.update(text);
        let hashed = hasher.finalize();
        let hashed = format!("{:x}.mp3", hashed);
        let file_path = path.join(hashed);
        if let Ok(file) = File::open(file_path) {
            Some(Box::new(file))
        } else {
            None
        }
    }

    fn set(&self, text: String, contents: Vec<u8>) -> Result<(), Box<dyn std::error::Error>> {
        let path = Path::new(&self.cache_dir_path);
        let mut hasher = Sha256::new();
        hasher.update(text);
        let hashed = hasher.finalize();
        let hashed = format!("{:x}.mp3", hashed);
        let file_path = path.join(hashed);
        let mut file = File::create(file_path)?;
        file.write_all(&contents)?;
        file.flush()?;
        Ok(())
    }
}

pub struct SpeechService {
    speech_client: google_tts::GoogleTtsClient,
    output_sink: rodio::Sink,
    audio_cache: Option<AudioCache>,
}

impl SpeechService {
    pub fn new(google_api_key: String, cache_dir_path: Option<String>) -> Result<SpeechService, Box<dyn std::error::Error>> {
        let client = google_tts::GoogleTtsClient::new(google_api_key);

        let output_device = rodio::default_output_device().ok_or("Failed to get default output device")?;
        info!("Started SpeechService with {}", output_device.name()?);
        let output_sink = rodio::Sink::new(&output_device);

        let audio_cache = match cache_dir_path {
            Some(path) => {
                Some(AudioCache::new(path)?)
            },
            None => None,
        };

        Ok(SpeechService {
            speech_client: client,
            output_sink,
            audio_cache,
        })
    }

    pub fn say(&self, text: String) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(audio_cache) = &self.audio_cache {
            if let Some(file) = audio_cache.get(text.clone()) {
                info!("Using cached value");
                self.output_sink.append(rodio::Decoder::new(file)?);
                
            } else {
                info!("Writing new file");
                let data = self.speech_client.synthesize(
                    google_tts::TextInput::with_text(text.clone()),
                    google_tts::VoiceProps::default_english_female_wavenet(),
                    google_tts::AudioConfig::default_with_encoding(google_tts::AudioEncoding::Mp3)
                )?;
                audio_cache.set(text.clone(), data.as_byte_stream()?)?;
                let buffer = Cursor::new(data.as_byte_stream()?);
                self.output_sink.append(rodio::Decoder::new(BufReader::new(buffer))?);
            }
            Ok(())
        } else {
            let data = self.speech_client.synthesize(
                google_tts::TextInput::with_text(text),
                google_tts::VoiceProps::default_english_female_wavenet(),
                google_tts::AudioConfig::default_with_encoding(google_tts::AudioEncoding::Mp3)
            )?;
    
            let buffer = Cursor::new(data.as_byte_stream()?);
            self.output_sink.append(rodio::Decoder::new(BufReader::new(buffer))?);
            Ok(())
        }
    }
}
