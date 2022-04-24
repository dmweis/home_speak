use crossbeam_channel::{unbounded, Sender};
use home_speak::speech_service::{SpeechService, TtsService};
use log::*;
use serde::Deserialize;
use simplelog::*;
use std::{io::Read, path::PathBuf, str};
use structopt::StructOpt;

#[derive(Deserialize, Debug, Clone)]
struct AppConfig {
    google_api_key: String,
    azure_api_key: String,
    cache_dir_path: Option<String>,
    tts_service: TtsService,
}

fn get_settings(config: Option<PathBuf>) -> Result<AppConfig, Box<dyn std::error::Error>> {
    let mut settings = config::Config::default();

    if let Some(config) = config {
        info!("Using configuration from {:?}", config);
        settings.merge(config::File::with_name(
            config.to_str().ok_or("Failed to convert path")?,
        ))?;
    } else {
        info!("Using default configuration");
        settings
            .merge(config::File::with_name("settings"))?
            .merge(config::File::with_name("dev_settings"))?;
    }

    settings.merge(config::Environment::with_prefix("APP"))?;

    Ok(settings.try_into()?)
}

#[derive(StructOpt, Debug)]
#[structopt(
    version = "0.1.0",
    author = "David M. Weis <dweis7@gmail.com>",
    about = "CLI tool for playing text to speech commands using Google text to speech cloud API"
)]
struct Opts {
    #[structopt()]
    phrases: String,
    #[structopt(long)]
    config: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logging();
    let opts = Opts::from_args();

    let app_config = get_settings(opts.config)?;

    let speech_service = SpeechService::new(
        app_config.google_api_key,
        &app_config.azure_api_key,
        app_config.cache_dir_path,
    )?;

    let speech_service_handle = start_speech_service_worker(speech_service, app_config.tts_service);

    let phrases = opts.phrases.split(',');
    for phrase in phrases.into_iter().filter(|text| !text.is_empty()) {
        speech_service_handle.say(phrase);
    }

    println!("Press Enter to exit...");
    let _ = std::io::stdin().read(&mut [0]).unwrap();
    Ok(())
}

#[derive(Debug, Clone)]
struct SpeechServiceHandle {
    sender: Sender<String>,
}

impl SpeechServiceHandle {
    pub fn say(&self, phrase: &str) {
        self.sender
            .send(phrase.to_owned())
            .expect("Speech service send failed");
    }
}

fn start_speech_service_worker(
    mut speech_service: SpeechService,
    tts_service: TtsService,
) -> SpeechServiceHandle {
    let (sender, r) = unbounded::<String>();

    tokio::spawn(async move {
        for msg in r {
            if let Err(e) = speech_service.say(&msg, tts_service).await {
                error!("Speech service error {}", e);
            }
        }
    });

    SpeechServiceHandle { sender }
}

fn setup_logging() {
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
}