use super::{
    router::Router,
    routes::{DoorSensorHandler, MotionSensorHandler, SayHandler, SayMoodHandler},
};
use crate::configuration::AppConfig;
use crate::speech_service::{AzureVoiceStyle, SpeechService};
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

        client
            .subscribe("zigbee2mqtt/motion_one", QoS::AtMostOnce)
            .await
            .unwrap();
        router.add_handler(
            "zigbee2mqtt/motion_one",
            MotionSensorHandler::new(speech_service.clone()),
        );

        client
            .subscribe("zigbee2mqtt/main_door", QoS::AtMostOnce)
            .await
            .unwrap();
        router.add_handler(
            "zigbee2mqtt/main_door",
            DoorSensorHandler::new(speech_service.clone()),
        );

        let say_route = format!("{}/say", base_topic);
        client.subscribe(&say_route, QoS::AtMostOnce).await.unwrap();
        router.add_handler(&say_route, SayHandler::new(speech_service.clone()));

        // moods
        let say_cheerful_route = format!("{}/say/cheerful", base_topic);
        client
            .subscribe(&say_cheerful_route, QoS::AtMostOnce)
            .await
            .unwrap();
        router.add_handler(
            &say_cheerful_route,
            SayMoodHandler::new(speech_service.clone(), AzureVoiceStyle::Cheerful),
        );

        let say_angry_route = format!("{}/say/angry", base_topic);
        client
            .subscribe(&say_angry_route, QoS::AtMostOnce)
            .await
            .unwrap();
        router.add_handler(
            &say_angry_route,
            SayMoodHandler::new(speech_service.clone(), AzureVoiceStyle::Angry),
        );

        let say_sad_route = format!("{}/say/sad", base_topic);
        client
            .subscribe(&say_sad_route, QoS::AtMostOnce)
            .await
            .unwrap();
        router.add_handler(
            &say_sad_route,
            SayMoodHandler::new(speech_service.clone(), AzureVoiceStyle::Sad),
        );

        let say_plain_route = format!("{}/say/plain", base_topic);
        client
            .subscribe(&say_plain_route, QoS::AtMostOnce)
            .await
            .unwrap();
        router.add_handler(
            &say_plain_route,
            SayMoodHandler::new(speech_service.clone(), AzureVoiceStyle::Plain),
        );

        loop {
            let message = message_receiver.recv().await.unwrap();
            match router
                .handle_message(message.topic.clone(), &message.payload)
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
