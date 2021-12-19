mod speech_service;

use bytes::Bytes;
use clap::Parser;
use crossbeam_channel::unbounded;
use log::*;
use simplelog::*;
use std::str;
use std::vec;
use warp::Filter;

use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
struct AppConfig {
    google_api_key: String,
    cache_dir_path: Option<String>,
}

fn get_settings() -> Result<AppConfig, Box<dyn std::error::Error>> {
    let mut settings = config::Config::default();
    settings
        .merge(config::File::with_name("settings"))?
        .merge(config::File::with_name("dev_settings"))?
        .merge(config::Environment::with_prefix("APP"))?;
    Ok(settings.try_into()?)
}

#[derive(Parser)]
#[clap(
    version = "0.1.0",
    author = "David M. Weis <dweis7@gmail.com>",
    about = "CLI tool for playing text to speech commands using Google text to speech cloud API"
)]
struct Opts;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if TermLogger::init(
        LevelFilter::Info,
        Config::default(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    )
    .is_err()
    {
        eprintln!("Failed to create term logger");
        if SimpleLogger::init(LevelFilter::Info, Config::default()).is_err() {
            eprintln!("Failed to create simple logger");
        }
    }

    let app_config = get_settings()?;

    let (s, r) = unbounded();
    let speech_service =
        speech_service::SpeechService::new(app_config.google_api_key, app_config.cache_dir_path)?;

    tokio::spawn(async move {
        for msg in r {
            speech_service.say(msg).await.unwrap();
        }
    });

    let phrases = vec![
        "Hi?",
        "How is life you nerd?",
        "This is me testing things!",
        "This works so well!",
    ];

    for phrase in phrases {
        s.send(String::from(phrase)).unwrap();
    }

    let rest_sender = s.clone();
    let route = warp::path("say")
        .and(warp::post())
        .and(warp::body::bytes())
        .map(move |payload: Bytes| {
            if let Ok(text) = str::from_utf8(&payload) {
                rest_sender.send(text.to_owned()).unwrap();
                "Ok\n"
            } else {
                error!("Failed processing rest request");
                "Error"
            }
        });
    warp::serve(route).run(([0, 0, 0, 0], 3000)).await;
    Ok(())
}
