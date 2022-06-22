use super::{
    router::Router,
    routes::{MotionSensorHandler, SayCheerfulHandler, SayHandler},
};
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

    let base_topic = app_config.mqtt.base_route;

    let (message_sender, mut message_receiver) = unbounded_channel();

    tokio::spawn(async move {
        loop {
            if let Ok(notification) = eventloop.poll().await {
                if let Event::Incoming(Incoming::Publish(publish)) = notification {
                    message_sender
                        .send(publish)
                        .expect("Failed to publish message");
                }
            } else {
                error!("failed processing mqtt notifications");
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

        let say_route = format!("{}/say", base_topic);
        client.subscribe(&say_route, QoS::AtMostOnce).await.unwrap();
        router.add_handler(&say_route, SayHandler::new(speech_service.clone()));

        let say_cheerful_route = format!("{}/say/cheerful", base_topic);
        client
            .subscribe(&say_cheerful_route, QoS::AtMostOnce)
            .await
            .unwrap();
        router.add_handler(&say_cheerful_route, SayCheerfulHandler::new(speech_service));

        loop {
            let message = message_receiver.recv().await.unwrap();
            match router
                .handle_message(message.topic.clone(), &message.payload)
                .await
            {
                Ok(false) => error!("no handler for topic: \"{}\"", &message.topic),
                Ok(true) => (),
                Err(e) => error!("Failed running handler with {:?}", e),
            }
        }
    });

    Ok(())
}
