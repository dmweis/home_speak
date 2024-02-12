use crate::speech_service::TtsService;
use secrecy::Secret;
use serde::Deserialize;
use std::{path::PathBuf, str};
use tracing::*;

/// Use default config if no path is provided
pub fn get_configuration(config: Option<PathBuf>) -> Result<AppConfig, anyhow::Error> {
    let mut settings_builder = config::Config::builder();

    if let Some(config) = config {
        info!("Using configuration from {:?}", config);
        settings_builder = settings_builder.add_source(config::File::with_name(
            config
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("Failed to convert path"))?,
        ));
    } else {
        info!("Using dev configuration");
        settings_builder = settings_builder
            .add_source(config::File::with_name("configuration/settings"))
            .add_source(config::File::with_name("configuration/dev_settings"));
    }

    settings_builder = settings_builder.add_source(config::Environment::with_prefix("APP"));

    Ok(settings_builder.build()?.try_deserialize::<AppConfig>()?)
}

#[derive(Deserialize, Debug, Clone)]
pub struct AppConfig {
    pub tts_service_config: TtsServiceConfig,
    #[serde(default)]
    pub skip_intro: bool,
    pub assistant_config: AssistantConfig,
    pub mqtt: MqttConfig,
}

#[derive(Deserialize, Debug, Clone)]
pub struct TtsServiceConfig {
    pub google_api_key: Secret<String>,
    pub azure_api_key: Secret<String>,
    pub eleven_labs_api_key: Secret<String>,
    pub cache_dir_path: Option<String>,
    pub tts_service: TtsService,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct AssistantConfig {
    pub name: String,
    pub primary_user_name: String,
}

// weird serde default thing
const DEFAULT_MQTT_PORT: u16 = 1883;

const fn default_mqtt_port() -> u16 {
    DEFAULT_MQTT_PORT
}

#[derive(Deserialize, Debug, Clone)]
pub struct MqttConfig {
    pub base_route: String,
    pub broker_host: String,
    #[serde(default = "default_mqtt_port")]
    pub broker_port: u16,
    pub client_id: String,
}
