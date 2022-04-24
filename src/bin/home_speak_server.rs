use bytes::Bytes;
use crossbeam_channel::{unbounded, Sender};
use home_speak::speech_service::{SpeechService, TtsService};
use local_ip_address::list_afinet_netifas;
use log::*;
use serde::Deserialize;
use simplelog::*;
use std::{path::PathBuf, str};
use structopt::StructOpt;
use warp::Filter;

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

    // TODO(David): Extract this
    // probably use some templateing engine too
    if let Ok(network_interfaces) = list_afinet_netifas() {
        let mut interfaces = String::new();
        for (name, ip) in network_interfaces.iter() {
            if ip.is_ipv4() && !ip.is_loopback() {
                let interface = format!("{} at {:?}. ", name, ip);
                interfaces.push_str(&interface);
            }
        }
        info!("local interfaces are: {:?}", interfaces);
        speech_service_handle.say(&format!(
            "Joy has woken up. I am currently reachable on following addresses. {}. My hostname is {}",
            interfaces,
            hostname(),
        ));
    } else {
        error!("Failed to query local network interfaces");
        speech_service_handle.say("Failed to query local network interfaces");
    }

    let rest_sender = speech_service_handle.clone();
    let route = warp::path("say")
        .and(warp::post())
        .and(warp::body::bytes())
        .map(move |payload: Bytes| {
            if let Ok(text) = str::from_utf8(&payload) {
                rest_sender.say(text);
                "Ok\n"
            } else {
                error!("Failed processing rest request");
                "Error"
            }
        });
    warp::serve(route).run(([0, 0, 0, 0], 3000)).await;
    Ok(())
}

// TODO(David): I can do better than this

#[cfg(target_os = "linux")]
fn hostname() -> String {
    use std::process::Command;

    let output = Command::new("hostname")
        .output()
        .expect("failed to execute process");
    str::from_utf8(&output.stdout).unwrap().to_owned()
}

#[cfg(not(target_os = "linux"))]
fn hostname() -> String {
    String::from("Unavailable on this platform")
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
