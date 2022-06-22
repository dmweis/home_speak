use crate::speech_service::{AzureVoiceStyle, SpeechService};
use async_trait::async_trait;
use log::*;
use rumqttc::{AsyncClient, Event, Incoming, MqttOptions, QoS};
use serde::Deserialize;
use std::{sync::Arc, time::Duration};
use tokio::sync::mpsc::unbounded_channel;
use tokio::sync::Mutex;

pub fn start_mqtt_service(speech_service: Arc<Mutex<SpeechService>>) -> anyhow::Result<()> {
    let mut mqttoptions = MqttOptions::new("home-speak", "homepi.local", 1883);
    mqttoptions.set_keep_alive(Duration::from_secs(5));

    let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);

    let (message_sender, mut message_receiver) = unbounded_channel();

    tokio::spawn(async move {
        loop {
            let notification = eventloop.poll().await.unwrap();
            if let Event::Incoming(Incoming::Publish(publish)) = notification {
                message_sender
                    .send(publish)
                    .expect("Failed to publish message");
            }
        }
    });

    tokio::spawn(async move {
        let mut router = Router::default();

        client
            .subscribe("zigbee2mqtt/motion_one", QoS::AtMostOnce)
            .await
            .unwrap();

        router.add_handler(
            "zigbee2mqtt/motion_one",
            MotionSensorHandler::new(speech_service),
        );

        loop {
            let message = message_receiver.recv().await.unwrap();
            if !router
                .handle_message(message.topic.clone(), &message.payload)
                .await
            {
                error!("no handler for {}", &message.topic);
            }
        }
    });

    Ok(())
}

#[async_trait]
trait RouteHandler: Send {
    async fn call(&mut self, topic: &str, content: &[u8]) -> anyhow::Result<()>;
}

struct MotionSensorHandler {
    speech_service: Arc<Mutex<SpeechService>>,
}

impl MotionSensorHandler {
    fn new(speech_service: Arc<Mutex<SpeechService>>) -> Box<Self> {
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

#[derive(Default)]
struct Router {
    table: std::collections::HashMap<String, Box<dyn RouteHandler>>,
}

impl Router {
    fn add_handler(&mut self, topic: &str, handler: Box<dyn RouteHandler>) {
        self.table.insert(String::from(topic), handler);
    }

    async fn handle_message(&mut self, topic: String, content: &[u8]) -> bool {
        if let Some(handler) = self.table.get_mut(&topic) {
            handler.call(&topic, content).await.unwrap();
            true
        } else {
            false
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
pub struct MotionSensorData {
    pub battery: i64,
    pub battery_low: bool,
    pub linkquality: i64,
    pub occupancy: bool,
    pub tamper: bool,
    pub voltage: i64,
}
