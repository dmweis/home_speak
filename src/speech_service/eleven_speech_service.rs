use anyhow::Context;
use anyhow::Result;
use log::*;
use secrecy::{ExposeSecret, Secret};
use sha2::{Digest, Sha256};
use std::io::Cursor;

use crate::audio_cache::AudioCache;
use crate::eleven_labs_client;
use crate::speech_service::Playable;

use super::AudioService;

// Used to invalidate old cache
const ELEVEN_LABS_FORMAT_VERSION: u32 = 5;

fn hash_eleven_labs_tts(text: &str, voice_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text);
    hasher.update(voice_id);
    hasher.update(ELEVEN_LABS_FORMAT_VERSION.to_be_bytes());
    let hashed = hasher.finalize();
    format!("eleven-{:x}", hashed)
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
