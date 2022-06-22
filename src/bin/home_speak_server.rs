#[cfg(feature = "hotreload")]
use actix_files::NamedFile;
use home_speak::{
    alarm_service::AlarmService,
    configuration::get_configuration,
    mqtt::start_mqtt_service,
    server::start_server,
    speech_service::{SpeechService, TtsService},
    template_messages::TemplateEngine,
};
use log::*;
use simplelog::*;
use std::{path::PathBuf, sync::Arc};
use structopt::StructOpt;
use tokio::sync::Mutex;

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
async fn main() -> anyhow::Result<()> {
    setup_logging();
    let opts = Opts::from_args();

    let app_config = get_configuration(opts.config)?;

    let mut speech_service = SpeechService::new(
        app_config.tts_service_config.google_api_key.clone(),
        app_config.tts_service_config.azure_api_key.clone(),
        app_config.tts_service_config.cache_dir_path.clone(),
    )?;

    let template_engine = TemplateEngine::new(
        app_config.assistant_config.clone(),
        app_config.server_config.port,
    );

    if !app_config.skip_intro {
        let startup_message = template_engine.startup_message();
        for message_part in startup_message {
            speech_service.say(&message_part, TtsService::Azure).await?;
        }
    }

    let speech_service = Arc::new(tokio::sync::Mutex::new(speech_service));

    let alarm_service = Arc::new(tokio::sync::Mutex::new(AlarmService::new(
        speech_service.clone(),
    )));

    if let Some(ref path) = app_config.alarm_config.save_file_path {
        if let Err(e) = alarm_service.lock().await.add_alarms_from_file(path).await {
            error!("Failed to read saved alarms from {} with error {}", path, e);
            return Err(anyhow::anyhow!("Failed to read saved alarms {}", e));
        }
    }

    start_mqtt_service(app_config.clone(), speech_service.clone())?;

    let template_engine = Arc::new(Mutex::new(template_engine));

    start_server(app_config, speech_service, alarm_service, template_engine).await?;

    Ok(())
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
