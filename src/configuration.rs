use crate::speech_service::TtsService;
use log::*;
use secrecy::Secret;
use serde::Deserialize;
use std::{path::PathBuf, str};

/// Use default config if no path is provided
pub fn get_configuration(config: Option<PathBuf>) -> Result<AppConfig, anyhow::Error> {
    let mut settings = config::Config::default();

    if let Some(config) = config {
        info!("Using configuration from {:?}", config);
        settings.merge(config::File::with_name(
            config
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("Failed to convert path"))?,
        ))?;
    } else {
        info!("Using default configuration");
        settings
            .merge(config::File::with_name("configuration/settings"))?
            .merge(config::File::with_name("configuration/dev_settings"))?;
    }

    settings.merge(config::Environment::with_prefix("APP"))?;

    Ok(settings.try_into()?)
}

#[derive(Deserialize, Debug, Clone)]
pub struct AppConfig {
    pub tts_service_config: TtsServiceConfig,
    pub server_config: ServerConfig,
    #[serde(default)]
    pub skip_intro: bool,
    #[serde(default)]
    pub alarm_config: AlarmConfig,
    pub assistant_config: AssistantConfig,
}

#[derive(Deserialize, Debug, Clone)]
pub struct TtsServiceConfig {
    pub google_api_key: Secret<String>,
    pub azure_api_key: Secret<String>,
    pub cache_dir_path: Option<String>,
    pub tts_service: TtsService,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct AlarmConfig {
    pub save_file_path: Option<String>,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct AssistantConfig {
    pub name: String,
    pub primary_user_name: String,
}
