#[cfg(feature = "hotreload")]
use actix_files::NamedFile;
use clap::Parser;
use home_speak::{
    audio_cache,
    configuration::get_configuration,
    error::HomeSpeakError,
    logging::{set_global_tracing_zenoh_subscriber, setup_tracing},
    mqtt::start_mqtt_service,
    speech_service::{
        AudioMessage, AudioRepository, AudioService, ElevenSpeechService, SpeechService, TtsService,
    },
    template_messages::TemplateEngine,
};
use rumqttc::AsyncClient;
use std::{path::PathBuf, sync::Arc};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};
use tracing::*;
use zenoh::prelude::r#async::*;

const MQTT_AUDIO_PUB_TOPIC: &str = "transcribed_audio";

#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Opts {
    #[clap(long)]
    config: Option<PathBuf>,

    /// Sets the level of verbosity
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opts = Opts::parse();
    setup_tracing(opts.verbose, "home-speak");

    let app_config = get_configuration(opts.config)?;

    // zenoh
    let zenoh_config = app_config.zenoh.get_zenoh_config()?;
    let zenoh_session = zenoh::open(zenoh_config)
        .res()
        .await
        .map_err(HomeSpeakError::ZenohError)?
        .into_arc();

    set_global_tracing_zenoh_subscriber(zenoh_session);

    let mqtt_base_topic = app_config.mqtt.base_route.clone();

    let (audio_sender, mut audio_receiver) = unbounded_channel();

    let audio_cache = if let Some(cache_dir_path) = &app_config.tts_service_config.cache_dir_path {
        audio_cache::AudioCache::new(cache_dir_path.clone())?
    } else {
        audio_cache::AudioCache::new_without_cache()
    };

    let audio_service = AudioService::new(Some(audio_sender))?;

    let mut speech_service = SpeechService::new_with_mqtt(
        app_config.tts_service_config.google_api_key.clone(),
        app_config.tts_service_config.azure_api_key.clone(),
        audio_cache.clone(),
        audio_service.clone(),
    )?;

    let eleven_speech_service = ElevenSpeechService::new(
        app_config.tts_service_config.eleven_labs_api_key.clone(),
        audio_cache,
        audio_service.clone(),
    )
    .await?;

    let audio_repository_service =
        AudioRepository::new(&app_config.audio_repository_path, audio_service.clone())?;

    let template_engine = TemplateEngine::new(app_config.assistant_config.clone());

    if !app_config.skip_intro {
        let startup_message = template_engine.startup_message();
        for message_part in startup_message {
            speech_service.say(&message_part, TtsService::Azure).await?;
        }
    }

    let speech_service = Arc::new(tokio::sync::Mutex::new(speech_service));

    // TODO: I can't pass the client to the speech service since the speech service needs to be passed here....
    let client = start_mqtt_service(
        app_config.clone(),
        speech_service.clone(),
        eleven_speech_service,
        audio_service,
        audio_repository_service,
    )?;

    let audio_worker_task = tokio::spawn(async move {
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

    audio_worker_task.await?;

    Ok(())
}
