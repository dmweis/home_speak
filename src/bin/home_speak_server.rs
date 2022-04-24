use actix_web::{post, web, App, HttpResponse, HttpServer, Responder};
use crossbeam_channel::{unbounded, Sender};
use home_speak::{
    configuration::get_configuration,
    speech_service::{SpeechService, TtsService},
    template_messages::{generate_startup_message, human_current_time},
};
use log::*;
use simplelog::*;
use std::{path::PathBuf, str};
use structopt::StructOpt;

#[post("/say")]
async fn say_handler(
    body: web::Bytes,
    speech_service_handle: web::Data<SpeechServiceHandle>,
) -> impl Responder {
    if let Ok(text) = str::from_utf8(&body) {
        speech_service_handle.say(text);
        HttpResponse::Ok().finish()
    } else {
        error!("Failed processing rest request");
        HttpResponse::BadRequest().finish()
    }
}

#[post("/sample_azure_languages")]
async fn sample_azure_languages_handler(
    body: web::Bytes,
    speech_service_handle: web::Data<SpeechServiceHandle>,
) -> impl Responder {
    if let Ok(text) = str::from_utf8(&body) {
        speech_service_handle.sample_azure_languages(text);
        HttpResponse::Ok().finish()
    } else {
        error!("Failed processing rest request");
        HttpResponse::BadRequest().finish()
    }
}

#[post("/intro")]
async fn intro_handler(
    speech_service_handle: web::Data<SpeechServiceHandle>,
    port: web::Data<BoundPort>,
) -> impl Responder {
    let startup_message = generate_startup_message(port.0);
    speech_service_handle.say(&startup_message);
    HttpResponse::Ok().finish()
}

#[post("/current_time")]
async fn current_time_handler(
    speech_service_handle: web::Data<SpeechServiceHandle>,
) -> impl Responder {
    let current_time = human_current_time();
    speech_service_handle.say(&format!("Current time is {}", current_time));
    HttpResponse::Ok().finish()
}

#[derive(Debug, Clone, Copy)]
struct BoundPort(u16);

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

    let app_config = get_configuration(opts.config)?;

    let speech_service = SpeechService::new(
        app_config.tts_service_config.google_api_key,
        app_config.tts_service_config.azure_api_key,
        app_config.tts_service_config.cache_dir_path,
    )?;

    let speech_service_handle =
        start_speech_service_worker(speech_service, app_config.tts_service_config.tts_service);

    if !app_config.skip_intro {
        let startup_message = generate_startup_message(app_config.server_config.port);
        speech_service_handle.say(&startup_message);
    }

    let speech_service_handle = web::Data::new(speech_service_handle);

    let address = format!(
        "{}:{}",
        app_config.server_config.host, app_config.server_config.port
    );

    HttpServer::new(move || {
        let port = web::Data::new(BoundPort(app_config.server_config.port));

        App::new()
            .service(intro_handler)
            .service(say_handler)
            .service(current_time_handler)
            .service(sample_azure_languages_handler)
            .app_data(speech_service_handle.clone())
            .app_data(port)
    })
    .bind(address)?
    .run()
    .await?;
    Ok(())
}

#[derive(Debug, Clone)]
struct SpeechServiceHandle {
    sender: Sender<SpeechServiceMessage>,
}

enum SpeechServiceMessage {
    Simple(String),
    AzureVoiceSampling(String),
}

impl SpeechServiceHandle {
    pub fn say(&self, phrase: &str) {
        self.sender
            .send(SpeechServiceMessage::Simple(phrase.to_owned()))
            .expect("Speech service send failed");
    }

    pub fn sample_azure_languages(&self, phrase: &str) {
        self.sender
            .send(SpeechServiceMessage::AzureVoiceSampling(phrase.to_owned()))
            .expect("Speech service send failed");
    }
}

fn start_speech_service_worker(
    mut speech_service: SpeechService,
    tts_service: TtsService,
) -> SpeechServiceHandle {
    let (sender, r) = unbounded::<SpeechServiceMessage>();

    tokio::spawn(async move {
        for msg in r {
            // speech is actually partially blocking
            // thought it doesn't have to be. It's just because of how we handle
            // waiting until a sample is done playing
            match msg {
                SpeechServiceMessage::Simple(message) => {
                    if let Err(e) = speech_service.say(&message, tts_service).await {
                        error!("Speech service error {}", e);
                    }
                }
                SpeechServiceMessage::AzureVoiceSampling(message) => {
                    if let Err(e) = speech_service.sample_azure_languages(&message).await {
                        error!("Speech service error {}", e);
                    }
                }
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
