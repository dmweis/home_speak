use super::router::RouteHandler;
use crate::speech_service::{AzureVoiceStyle, SpeechService};
use async_trait::async_trait;
use log::*;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::Mutex;

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

#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
struct MotionSensorData {
    pub battery: i64,
    pub battery_low: bool,
    pub linkquality: i64,
    pub occupancy: bool,
    pub tamper: bool,
    pub voltage: i64,
}
