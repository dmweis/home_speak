use crate::speech_service::TtsService;
use log::*;
use serde::Deserialize;
use std::{path::PathBuf, str};

/// Use default config if no path is provided
pub fn get_configuration(config: Option<PathBuf>) -> Result<AppConfig, Box<dyn std::error::Error>> {
    let mut settings = config::Config::default();

    if let Some(config) = config {
        info!("Using configuration from {:?}", config);
        settings.merge(config::File::with_name(
            config.to_str().ok_or("Failed to convert path")?,
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
    pub google_api_key: String,
    pub azure_api_key: String,
    pub cache_dir_path: Option<String>,
    pub tts_service: TtsService,
}
