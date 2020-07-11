mod speech_service;

use clap::Clap;
use std::env;
use rumqtt::{MqttClient, MqttOptions, QoS, ReconnectOptions, Notification};
use log::*;
use simplelog::*;
use std::str;
use crossbeam_channel::{ unbounded, Sender };
use warp::Filter;
use bytes::Bytes;


fn mqtt_service(sender: Sender<String>) {

    let mqtt_options = MqttOptions::new("home_speak", "mqtt.local", 1883)
        .set_reconnect_opts(ReconnectOptions::Always(5));

    let (mut mqtt_client, notifications) = MqttClient::start(mqtt_options).expect("Failed to connect to MQTT host");

    mqtt_client.subscribe("home/say", QoS::AtLeastOnce).expect("Failed to subscribe to channel");
    mqtt_client.subscribe("discord/receive/722904321108213871", QoS::AtLeastOnce).expect("Failed to subscribe to channel");
    for notification in notifications {
        match notification {
            Notification::Publish(message) => {
                trace!("New message");
                match str::from_utf8(&message.payload) {
                    Ok(message_text) => sender.send(message_text.to_owned()).unwrap(),
                    Err(error) => error!("Error while trying to play message {}", error),
                }
            },
            Notification::Disconnection => {
                warn!("Client lost connection");
            },
            Notification::Reconnection => {
                warn!("client reconnected");
                mqtt_client.subscribe("home/say", QoS::AtLeastOnce).expect("Failed to subscribe to channel");
                mqtt_client.subscribe("discord/receive/722904321108213871", QoS::AtLeastOnce).expect("Failed to subscribe to channel");
            },
            other => {
                warn!("Unexpected message {:?}", other);
            }
        }
    }
}


#[derive(Clap)]
#[clap(
    version = "0.1.0",
    author = "David M. Weis <dweis7@gmail.com>",
    about = "CLI tool for playing text to speech commands using Google text to speech cloud API"
)]
struct Opts {
    #[clap(
        short = "c",
        long = "cache-dir",
        about = "Path to caching directory"
    )]
    cache_dir_path: Option<String>
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = ConfigBuilder::new()
        .add_filter_allow_str(env!("CARGO_PKG_NAME"))
        .build();
    if TermLogger::init(LevelFilter::Info, config.clone(), TerminalMode::Mixed).is_err() {
        eprintln!("Failed to create term logger");
        if SimpleLogger::init(LevelFilter::Info, config).is_err() {
            eprintln!("Failed to create simple logger");
        }
    }

    let google_api_key = env::var("GOOGLE_API_KEY").expect("Please set GOOGLE_API_KEY");
    let args: Opts = Opts::parse();

    let (s, r) = unbounded();
    let speech_service = speech_service::SpeechService::new(google_api_key, args.cache_dir_path)?;

    tokio::spawn(async move {
        for msg in r {
            speech_service.say(msg).await.unwrap();
        }
    });

    let mqtt_sender = s.clone();
    std::thread::spawn(move || {
        mqtt_service(mqtt_sender);
    });

    let rest_sender = s.clone();
    let route = warp::path("say")
    .and(warp::post())
    .and(warp::body::bytes())
    .map(move |payload: Bytes| {
        if let Ok(text) = str::from_utf8(&payload) {
            rest_sender.send(text.to_owned()).unwrap();
            "Ok"
        } else {
            error!("Failed processing rest request");
            "Error"
        }
    });
    warp::serve(route).run(([0, 0, 0, 0], 3000)).await;
    Ok(())
}
