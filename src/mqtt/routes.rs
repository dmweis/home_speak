use crate::{
    speech_service::{AzureVoiceStyle, SpeechService},
    template_messages::TemplateEngine,
};
use async_trait::async_trait;
use log::*;
use mqtt_router::{RouteHandler, RouterError};
use serde::Deserialize;
use std::{str::from_utf8, sync::Arc};
use tokio::sync::Mutex;

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
    async fn call(&mut self, _topic: &str, content: &[u8]) -> std::result::Result<(), RouterError> {
        info!("mqtt say command");
        let command: SayCommand =
            serde_json::from_slice(content).map_err(|err| RouterError::HandlerError(err.into()))?;

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
    async fn call(&mut self, _topic: &str, content: &[u8]) -> std::result::Result<(), RouterError> {
        info!("mqtt say cheerful command");
        let message = from_utf8(content).map_err(|err| RouterError::HandlerError(err.into()))?;

        match self
            .speech_service
            .lock()
            .await
            .say_azure_with_style(message, self.style)
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
