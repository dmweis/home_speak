use super::{router::Router, routes::MotionSensorHandler};
use crate::configuration::AppConfig;
use crate::speech_service::SpeechService;
use log::*;
use rumqttc::{AsyncClient, Event, Incoming, MqttOptions, QoS};
use std::{sync::Arc, time::Duration};
use tokio::sync::mpsc::unbounded_channel;
use tokio::sync::Mutex;

pub fn start_mqtt_service(
    app_config: AppConfig,

    speech_service: Arc<Mutex<SpeechService>>,
) -> anyhow::Result<()> {
    let mut mqttoptions = MqttOptions::new(
        &app_config.mqtt.client_id,
        &app_config.mqtt.broker_host,
        app_config.mqtt.broker_port,
    );
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
                .unwrap()
            {
                error!("no handler for {}", &message.topic);
            }
        }
    });

    Ok(())
}
