use crate::{
    speech_service::{
        AudioRepository, AudioService, AzureVoiceStyle, ElevenSpeechService, SpeechService,
    },
    template_messages::TemplateEngine,
};
use anyhow::Context;
use async_trait::async_trait;
use mqtt_router::RouteHandler;
use serde::Deserialize;
use std::{io::Cursor, str::from_utf8, sync::Arc};
use tokio::sync::Mutex;
use tracing::*;

pub struct SayHandler {
    speech_service: Arc<Mutex<SpeechService>>,
}

impl SayHandler {
    pub fn new(speech_service: Arc<Mutex<SpeechService>>) -> Box<Self> {
        Box::new(Self { speech_service })
    }
}

#[async_trait]
impl RouteHandler for SayHandler {
    #[instrument(skip(self, content))]
    async fn call(
        &mut self,
        _topic: &str,
        content: &[u8],
    ) -> std::result::Result<(), anyhow::Error> {
        info!("mqtt say command");
        let command: SayCommand = serde_json::from_slice(content)?;

        let message = if command.template {
            TemplateEngine::template_substitute(&command.content)
        } else {
            command.content.clone()
        };

        match self
            .speech_service
            .lock()
            .await
            .say_azure_with_style(&message, command.style)
            .await
        {
            Ok(_) => (),
            Err(e) => {
                error!("Failed to call speech service {}", e);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct SayCommand {
    content: String,
    style: AzureVoiceStyle,
    #[serde(default)]
    template: bool,
}

pub struct SayMoodHandler {
    speech_service: Arc<Mutex<SpeechService>>,
    style: AzureVoiceStyle,
}

impl SayMoodHandler {
    pub fn new(speech_service: Arc<Mutex<SpeechService>>, style: AzureVoiceStyle) -> Box<Self> {
        Box::new(Self {
            speech_service,
            style,
        })
    }
}

#[async_trait]
impl RouteHandler for SayMoodHandler {
    #[instrument(skip(self, content))]
    async fn call(
        &mut self,
        _topic: &str,
        content: &[u8],
    ) -> std::result::Result<(), anyhow::Error> {
        info!("mqtt say cheerful command");
        let message = from_utf8(content)?;

        match self
            .speech_service
            .lock()
            .await
            .say_azure_with_style(message, self.style)
            .await
        {
            Ok(_) => (),
            Err(e) => {
                error!("Failed to call speech service {:?}", e);
            }
        }
        Ok(())
    }
}

pub struct SayElevenDefaultHandler {
    speech_service: ElevenSpeechService,
}

impl SayElevenDefaultHandler {
    pub fn new(speech_service: ElevenSpeechService) -> Box<Self> {
        Box::new(Self { speech_service })
    }
}

#[async_trait]
impl RouteHandler for SayElevenDefaultHandler {
    #[instrument(skip(self, content))]
    async fn call(
        &mut self,
        _topic: &str,
        content: &[u8],
    ) -> std::result::Result<(), anyhow::Error> {
        info!("mqtt say eleven command");
        let message = from_utf8(content)?;

        match self
            .speech_service
            .say_eleven_with_default_voice(message)
            .await
        {
            Ok(_) => (),
            Err(e) => {
                error!("Failed to call speech service {:?}", e);
            }
        }
        Ok(())
    }
}

pub struct SayElevenCustomVoiceHandler {
    speech_service: ElevenSpeechService,
}

impl SayElevenCustomVoiceHandler {
    pub fn new(speech_service: ElevenSpeechService) -> Box<Self> {
        Box::new(Self { speech_service })
    }
}

#[async_trait]
impl RouteHandler for SayElevenCustomVoiceHandler {
    #[instrument(skip(self, content))]
    async fn call(
        &mut self,
        topic: &str,
        content: &[u8],
    ) -> std::result::Result<(), anyhow::Error> {
        let voice_name = topic
            .split('/')
            .last()
            .context("Failed to extract voice name")?;

        info!("mqtt say eleven custom voice command: {}", voice_name);

        let message = from_utf8(content)?;

        match self.speech_service.say_eleven(message, voice_name).await {
            Ok(_) => (),
            Err(e) => {
                error!("Failed to call speech service {:?}", e);
            }
        }
        Ok(())
    }
}

pub struct Mp3AudioPlayerHandler {
    audio_service: AudioService,
}

impl Mp3AudioPlayerHandler {
    pub fn new(audio_service: AudioService) -> Box<Self> {
        Box::new(Self { audio_service })
    }
}

#[async_trait]
impl RouteHandler for Mp3AudioPlayerHandler {
    #[instrument(skip(self, content))]
    async fn call(
        &mut self,
        _topic: &str,
        content: &[u8],
    ) -> std::result::Result<(), anyhow::Error> {
        info!("mqtt mp3 audio player");

        let audio = Box::new(Cursor::new(content.to_vec()));
        if let Err(e) = self.audio_service.play(audio) {
            error!("Failed to call audio service {:?}", e);
        }
        Ok(())
    }
}

pub struct RestartRequestHandler {
    audio_service: AudioService,
}

impl RestartRequestHandler {
    pub fn new(audio_service: AudioService) -> Box<Self> {
        Box::new(Self { audio_service })
    }
}

#[async_trait]
impl RouteHandler for RestartRequestHandler {
    #[instrument(skip(self, _content))]
    async fn call(
        &mut self,
        _topic: &str,
        _content: &[u8],
    ) -> std::result::Result<(), anyhow::Error> {
        info!("mqtt mp3 audio player");

        match self.audio_service.restart_player() {
            Ok(_) => (),
            Err(e) => {
                error!("Failed to call audio service {:?}", e);
            }
        }
        Ok(())
    }
}

pub struct PlayAudioFileHandler {
    audio_repository: AudioRepository,
}

impl PlayAudioFileHandler {
    pub fn new(audio_service: AudioRepository) -> Box<Self> {
        Box::new(Self {
            audio_repository: audio_service,
        })
    }
}

#[async_trait]
impl RouteHandler for PlayAudioFileHandler {
    #[instrument(skip(self, content))]
    async fn call(
        &mut self,
        _topic: &str,
        content: &[u8],
    ) -> std::result::Result<(), anyhow::Error> {
        let file_path = std::str::from_utf8(content)?;
        info!("Playing {:?}", file_path);
        match self.audio_repository.play_file(file_path) {
            Ok(_) => (),
            Err(e) => {
                error!("Failed to play audio file {:?}", e);
            }
        }
        Ok(())
    }
}
