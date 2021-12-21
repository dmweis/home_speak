mod speech_service;

use bytes::Bytes;
use crossbeam_channel::unbounded;
use log::*;
use serde::Deserialize;
use simplelog::*;
use std::io::Read;
use std::str;
use structopt::StructOpt;
use warp::Filter;

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

#[derive(StructOpt, Debug)]
#[structopt(
    version = "0.1.0",
    author = "David M. Weis <dweis7@gmail.com>",
    about = "CLI tool for playing text to speech commands using Google text to speech cloud API"
)]
struct Opts {
    #[structopt(short, long)]
    phrases: Option<String>,
}

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
    let opts = Opts::from_args();

    let app_config = get_settings()?;

    let (s, r) = unbounded();
    let speech_service =
        speech_service::SpeechService::new(app_config.google_api_key, app_config.cache_dir_path)?;

    tokio::spawn(async move {
        for msg in r {
            speech_service.say(msg).await.unwrap();
        }
    });

    if let Some(phrases) = opts.phrases {
        let phrases = phrases.split(',');
        for phrase in phrases.into_iter().filter(|text| !text.is_empty()) {
            s.send(String::from(phrase)).unwrap();
        }

        println!("Press Enter to exit...");
        let _ = std::io::stdin().read(&mut [0]).unwrap();
    } else {
        // use rest service if no phrases provided
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
    }
    Ok(())
}
