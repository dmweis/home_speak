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
    async fn call(&mut self, _topic: &str, content: &[u8]) -> std::result::Result<(), RouterError> {
        info!("Handling motion sensor data");
        let motion_sensor: MotionSensorData =
            serde_json::from_slice(content).map_err(|err| RouterError::HandlerError(err.into()))?;

        let message = if motion_sensor.occupancy {
            "Motion sensor triggered"
        } else {
            "Motion sensor detects no movement"
        };

        self.speech_service
            .lock()
            .await
            .say_azure_with_style(message, AzureVoiceStyle::Cheerful)
            .await
            .map_err(|err| RouterError::HandlerError(err.into()))?;
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

pub struct DoorSensorHandler {
    speech_service: Arc<Mutex<SpeechService>>,
}

impl DoorSensorHandler {
    pub fn new(speech_service: Arc<Mutex<SpeechService>>) -> Box<Self> {
        Box::new(Self { speech_service })
    }
}

#[async_trait]
impl RouteHandler for DoorSensorHandler {
    async fn call(&mut self, _topic: &str, content: &[u8]) -> std::result::Result<(), RouterError> {
        info!("Handling door sensor data");
        let motion_sensor: DoorSensor =
            serde_json::from_slice(content).map_err(|err| RouterError::HandlerError(err.into()))?;

        let message = if motion_sensor.contact {
            "Front door closed"
        } else {
            "Front door opened"
        };

        self.speech_service
            .lock()
            .await
            .say_azure_with_style(message, AzureVoiceStyle::Cheerful)
            .await
            .map_err(|err| RouterError::HandlerError(err.into()))?;
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct DoorSensor {
    #[allow(dead_code)]
    pub battery: i64,
    #[allow(dead_code)]
    pub battery_low: bool,
    pub contact: bool,
    #[allow(dead_code)]
    pub linkquality: i64,
    #[allow(dead_code)]
    pub tamper: bool,
    #[allow(dead_code)]
    pub voltage: i64,
}

pub struct SwitchHandler {
    speech_service: Arc<Mutex<SpeechService>>,
}

impl SwitchHandler {
    pub fn new(speech_service: Arc<Mutex<SpeechService>>) -> Box<Self> {
        Box::new(Self { speech_service })
    }
}

#[async_trait]
impl RouteHandler for SwitchHandler {
    async fn call(&mut self, topic: &str, content: &[u8]) -> std::result::Result<(), RouterError> {
        info!("Handling switch data");
        let switch_name = topic.split('/').last().unwrap_or("unknown");
        let switch_data: SwitchPayload =
            serde_json::from_slice(content).map_err(|err| RouterError::HandlerError(err.into()))?;

        let message = match switch_data.action {
            Action::Single => format!("{switch_name} was clicked once"),
            Action::Long => format!("{switch_name} was long pressed"),
            Action::Double => format!("{switch_name} was double clicked"),
        };

        self.speech_service
            .lock()
            .await
            .say_azure_with_style(&message, AzureVoiceStyle::Cheerful)
            .await
            .map_err(|err| RouterError::HandlerError(err.into()))?;
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Single,
    Double,
    Long,
}

#[derive(Debug, Deserialize)]
pub struct SwitchPayload {
    pub action: Action,
    #[allow(dead_code)]
    pub battery: i64,
    #[allow(dead_code)]
    pub linkquality: i64,
    #[allow(dead_code)]
    pub voltage: i64,
}
