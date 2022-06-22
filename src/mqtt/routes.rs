use super::router::RouteHandler;
use crate::{
    speech_service::{AzureVoiceStyle, SpeechService},
    template_messages::TemplateEngine,
};
use async_trait::async_trait;
use log::*;
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
    async fn call(&mut self, _topic: &str, content: &[u8]) -> anyhow::Result<()> {
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

pub struct SayCheerfulHandler {
    speech_service: Arc<Mutex<SpeechService>>,
}

impl SayCheerfulHandler {
    pub fn new(speech_service: Arc<Mutex<SpeechService>>) -> Box<Self> {
        Box::new(Self { speech_service })
    }
}

#[async_trait]
impl RouteHandler for SayCheerfulHandler {
    async fn call(&mut self, _topic: &str, content: &[u8]) -> anyhow::Result<()> {
        info!("mqtt say cheerful command");
        let message = from_utf8(content)?;

        match self
            .speech_service
            .lock()
            .await
            .say_azure_with_style(message, AzureVoiceStyle::Cheerful)
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

pub struct MotionSensorHandler {
    speech_service: Arc<Mutex<SpeechService>>,
}

impl MotionSensorHandler {
    pub fn new(speech_service: Arc<Mutex<SpeechService>>) -> Box<Self> {
        Box::new(Self { speech_service })
    }
}

#[async_trait]
impl RouteHandler for MotionSensorHandler {
    async fn call(&mut self, _topic: &str, content: &[u8]) -> anyhow::Result<()> {
        info!("Handling motion sensor data");
        let motion_sensor: MotionSensorData = serde_json::from_slice(content)?;

        let message = if motion_sensor.occupancy {
            "Motion sensor triggered"
        } else {
            "Motion sensor detects no movement"
        };

        self.speech_service
            .lock()
            .await
            .say_azure_with_style(message, AzureVoiceStyle::Cheerful)
            .await?;
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct MotionSensorData {
    #[allow(dead_code)]
    pub battery: i64,
    #[allow(dead_code)]
    pub battery_low: bool,
    #[allow(dead_code)]
    pub linkquality: i64,
    pub occupancy: bool,
    #[allow(dead_code)]
    pub tamper: bool,
    #[allow(dead_code)]
    pub voltage: i64,
}
