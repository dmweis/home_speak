mod speech_service;

use bytes::Bytes;
use clap::Clap;
use crossbeam_channel::unbounded;
use log::*;
use simplelog::*;
use std::env;
use std::str;
use std::vec;
use warp::Filter;

#[derive(Clap)]
#[clap(
    version = "0.1.0",
    author = "David M. Weis <dweis7@gmail.com>",
    about = "CLI tool for playing text to speech commands using Google text to speech cloud API"
)]
struct Opts {
    #[clap(short = "c", long = "cache-dir", about = "Path to caching directory")]
    cache_dir_path: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // let config = ConfigBuilder::new()
    //     .add_filter_allow_str(env!("CARGO_PKG_NAME"))
    //     .build();
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

    let google_api_key = env::var("GOOGLE_API_KEY").expect("Please set GOOGLE_API_KEY");
    let args: Opts = Opts::parse();

    let (s, r) = unbounded();
    let speech_service = speech_service::SpeechService::new(google_api_key, args.cache_dir_path)?;

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
