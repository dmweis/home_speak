#[cfg(feature = "hotreload")]
use actix_files::NamedFile;
use clap::Parser;
use home_speak::{
    configuration::get_configuration,
    mqtt::start_mqtt_service,
    speech_service::{AudioMessage, SpeechService, TtsService},
    template_messages::TemplateEngine,
};
use log::*;
use rumqttc::AsyncClient;
use simplelog::*;
use std::{path::PathBuf, sync::Arc};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

const MQTT_AUDIO_PUB_TOPIC: &str = "transcribed_audio";

#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Opts {
    #[clap(long)]
    config: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    setup_logging();
    let opts = Opts::parse();

    let app_config = get_configuration(opts.config)?;

    let mqtt_base_topic = app_config.mqtt.base_route.clone();

    let (audio_sender, mut audio_receiver) = unbounded_channel();

    let mut speech_service = SpeechService::new_with_mqtt(
        app_config.tts_service_config.google_api_key.clone(),
        app_config.tts_service_config.azure_api_key.clone(),
        app_config.tts_service_config.eleven_labs_api_key.clone(),
        app_config.tts_service_config.cache_dir_path.clone(),
        Some(audio_sender),
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

    // TODO: I can't pass the client to the speech service since the speech service needs to be passed here....
    let client = start_mqtt_service(app_config.clone(), speech_service.clone())?;

    tokio::spawn(async move {
        async fn helper(
            audio_receiver: &mut UnboundedReceiver<AudioMessage>,
            mqtt_base_topic: &str,
            client: &AsyncClient,
        ) -> anyhow::Result<()> {
            if let Some(message) = audio_receiver.recv().await {
                let message = serde_json::to_string_pretty(&message).unwrap();
                let topic = format!("{mqtt_base_topic}/{MQTT_AUDIO_PUB_TOPIC}");
                client
                    .publish(topic, rumqttc::QoS::AtMostOnce, false, message)
                    .await?;
            }
            Ok(())
        }
        loop {
            if let Err(error) = helper(&mut audio_receiver, &mqtt_base_topic, &client).await {
                error!("Audio sender failed with {}", error);
            }
        }
    });

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
