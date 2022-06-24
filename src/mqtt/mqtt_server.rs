use super::routes::{
    DoorSensorHandler, MotionSensorHandler, SayHandler, SayMoodHandler, SwitchHandler,
};
use crate::{
    configuration::AppConfig,
    speech_service::{AzureVoiceStyle, SpeechService},
};
use log::*;
use mqtt_router::Router;
use rumqttc::{AsyncClient, Event, Incoming, MqttOptions, QoS, SubscribeFilter};
use std::{sync::Arc, time::Duration};
use tokio::sync::{mpsc::unbounded_channel, Mutex};

pub fn start_mqtt_service(
    app_config: AppConfig,

    speech_service: Arc<Mutex<SpeechService>>,
) -> anyhow::Result<()> {
    let mut mqttoptions = MqttOptions::new(
        &app_config.mqtt.client_id,
        &app_config.mqtt.broker_host,
        app_config.mqtt.broker_port,
    );
    info!("Starting MQTT server with options {:?}", mqttoptions);
    mqttoptions.set_keep_alive(Duration::from_secs(5));

    let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);

    let base_topic = app_config.mqtt.base_route;

    info!("MQTT base topic {}", base_topic);

    let (message_sender, mut message_receiver) = unbounded_channel();

    tokio::spawn(async move {
        loop {
            match eventloop.poll().await {
                Ok(notification) => {
                    if let Event::Incoming(Incoming::Publish(publish)) = notification {
                        if let Err(e) = message_sender.send(publish) {
                            error!("Error sending message {}", e);
                        }
                    }
                }
                Err(e) => {
                    error!("Error processing eventloop notifications {}", e);
                }
            }
        }
    });

    tokio::spawn(async move {
        let mut router = Router::default();

        // router
        //     .add_handler(
        //         "zigbee2mqtt/motion/one",
        //         MotionSensorHandler::new(speech_service.clone()),
        //     )
        //     .unwrap();

        router
            .add_handler(
                "zigbee2mqtt/main_door",
                DoorSensorHandler::new(speech_service.clone()),
            )
            .unwrap();

        router
            .add_handler(
                "zigbee2mqtt/switch/one",
                SwitchHandler::new(speech_service.clone()),
            )
            .unwrap();

        router
            .add_handler(
                "zigbee2mqtt/switch/two",
                SwitchHandler::new(speech_service.clone()),
            )
            .unwrap();

        // mood routers
        router
            .add_handler(
                &format!("{}/say", base_topic),
                SayHandler::new(speech_service.clone()),
            )
            .unwrap();

        router
            .add_handler(
                &format!("{}/say/cheerful", base_topic),
                SayMoodHandler::new(speech_service.clone(), AzureVoiceStyle::Cheerful),
            )
            .unwrap();

        router
            .add_handler(
                &format!("{}/say/angry", base_topic),
                SayMoodHandler::new(speech_service.clone(), AzureVoiceStyle::Angry),
            )
            .unwrap();

        router
            .add_handler(
                &format!("{}/say/sad", base_topic),
                SayMoodHandler::new(speech_service.clone(), AzureVoiceStyle::Sad),
            )
            .unwrap();

        router
            .add_handler(
                &format!("{}/say/plain", base_topic),
                SayMoodHandler::new(speech_service.clone(), AzureVoiceStyle::Plain),
            )
            .unwrap();

        let topics = router
            .topics_for_subscription()
            .map(|topic| SubscribeFilter {
                path: topic.to_owned(),
                qos: QoS::AtMostOnce,
            });
        client.subscribe_many(topics).await.unwrap();

        loop {
            let message = message_receiver.recv().await.unwrap();
            match router
                .handle_message_ignore_errors(&message.topic, &message.payload)
                .await
            {
                Ok(false) => error!("No handler for topic: \"{}\"", &message.topic),
                Ok(true) => (),
                Err(e) => error!("Failed running handler with {:?}", e),
            }
        }
    });

    Ok(())
}
