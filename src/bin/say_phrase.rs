use clap::Parser;
use crossbeam_channel::{unbounded, Sender};
use home_speak::{
    audio_cache,
    configuration::get_configuration,
    speech_service::{AudioService, SpeechService, TtsService},
};
use std::{io::Read, path::PathBuf, str};
use tracing::*;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Opts {
    phrases: String,
    #[clap(long)]
    config: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logging();
    let opts = Opts::parse();

    let app_config = get_configuration(opts.config)?;

    let audio_cache = if let Some(cache_dir_path) = &app_config.tts_service_config.cache_dir_path {
        audio_cache::AudioCache::new(cache_dir_path.clone())?
    } else {
        audio_cache::AudioCache::new_without_cache()
    };

    let audio_service = AudioService::new(None)?;

    let speech_service = SpeechService::new(
        app_config.tts_service_config.google_api_key,
        app_config.tts_service_config.azure_api_key,
        audio_cache,
        audio_service,
    )?;

    let speech_service_handle =
        start_speech_service_worker(speech_service, app_config.tts_service_config.tts_service);

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
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();
}
