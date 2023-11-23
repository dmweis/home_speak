use super::routes::{SayHandler, SayMoodHandler};
use crate::{
    configuration::AppConfig,
    mqtt::routes::{
        Mp3AudioPlayerHandler, RestartRequestHandler, SayElevenCustomVoiceHandler,
        SayElevenDefaultHandler,
    },
    speech_service::{AudioService, AzureVoiceStyle, ElevenSpeechService, SpeechService},
};
use log::*;
use mqtt_router::Router;
use rumqttc::{AsyncClient, ConnAck, Event, Incoming, MqttOptions, Publish, QoS, SubscribeFilter};
use std::{sync::Arc, time::Duration};
use tokio::sync::{mpsc::unbounded_channel, Mutex};

enum MqttUpdate {
    Message(Publish),
    Reconnection(ConnAck),
}

const MQTT_MAX_PACKET_SIZE: usize = 268435455;

pub fn start_mqtt_service(
    app_config: AppConfig,
    speech_service: Arc<Mutex<SpeechService>>,
    eleven_speech_service: ElevenSpeechService,
    audio_service: AudioService,
) -> anyhow::Result<AsyncClient> {
    let mut mqttoptions = MqttOptions::new(
        &app_config.mqtt.client_id,
        &app_config.mqtt.broker_host,
        app_config.mqtt.broker_port,
    );
    info!("Starting MQTT server with options {:?}", mqttoptions);
    mqttoptions.set_keep_alive(Duration::from_secs(5));
    mqttoptions.set_max_packet_size(MQTT_MAX_PACKET_SIZE, MQTT_MAX_PACKET_SIZE);

    let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);
    let client_clone = client.clone();

    let base_topic = app_config.mqtt.base_route;

    info!("MQTT base topic {}", base_topic);

    let (message_sender, mut message_receiver) = unbounded_channel();

    tokio::spawn(async move {
        loop {
            match eventloop.poll().await {
                Ok(notification) => match notification {
                    Event::Incoming(Incoming::Publish(publish)) => {
                        if let Err(e) = message_sender.send(MqttUpdate::Message(publish)) {
                            eprintln!("Error sending message {}", e);
                        }
                    }
                    Event::Incoming(Incoming::ConnAck(con_ack)) => {
                        if let Err(e) = message_sender.send(MqttUpdate::Reconnection(con_ack)) {
                            eprintln!("Error sending message {}", e);
                        }
                    }
                    _ => (),
                },
                Err(e) => {
                    eprintln!("Error processing eventloop notifications {}", e);
                }
            }
        }
    });

    tokio::spawn(async move {
        let mut router = Router::default();

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

        router
            .add_handler(
                &format!("{}/say/eleven/simple", base_topic),
                SayElevenDefaultHandler::new(eleven_speech_service.clone()),
            )
            .unwrap();

        router
            .add_handler(
                &format!("{}/say/eleven/voice/+", base_topic),
                SayElevenCustomVoiceHandler::new(eleven_speech_service.clone()),
            )
            .unwrap();

        router
            .add_handler(
                &format!("{}/play", base_topic),
                Mp3AudioPlayerHandler::new(audio_service.clone()),
            )
            .unwrap();

        router
            .add_handler(
                &format!("{}/restart", base_topic),
                RestartRequestHandler::new(audio_service.clone()),
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
            let update = message_receiver.recv().await.unwrap();
            match update {
                MqttUpdate::Message(message) => {
                    match router
                        .handle_message_ignore_errors(&message.topic, &message.payload)
                        .await
                    {
                        Ok(false) => error!("No handler for topic: \"{}\"", &message.topic),
                        Ok(true) => (),
                        Err(e) => error!("Failed running handler with {:?}", e),
                    }
                }
                MqttUpdate::Reconnection(_) => {
                    let topics = router
                        .topics_for_subscription()
                        .map(|topic| SubscribeFilter {
                            path: topic.to_owned(),
                            qos: QoS::AtMostOnce,
                        });
                    client.subscribe_many(topics).await.unwrap();
                }
            }
        }
    });

    Ok(client_clone)
}
