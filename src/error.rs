use chrono::format::ParseError;
use rumqttc::ClientError;
use thiserror::Error;

pub(crate) type Result<T> = std::result::Result<T, HomeSpeakError>;

#[derive(Error, Debug)]
pub enum HomeSpeakError {
    #[error("io error")]
    IoError(#[from] std::io::Error),
    #[error("cache dir path does not exist")]
    CacheDirPathNotFound,
    #[error("failed to decode audio file")]
    FailedToDecodeAudioFile,
    #[error("failed to create an audio sink")]
    FailedToCreateASink,
    #[error("failed to create an output stream")]
    FailedToCreateAnOutputStream,
    // TODO: Propagate errors from google_tts
    #[error("google tts failed to synthesize")]
    GoogleTtsError,
    #[error("azure tts error")]
    AzureTtsError(#[from] azure_tts::TtsError),
    #[error("serialisation error")]
    SerializationError(#[from] serde_json::Error),
    #[error("time format parse error")]
    TimeFormatParseError(#[from] ParseError),
    #[error("mqtt client error")]
    MqttClientError(#[from] ClientError),
    #[error("audio channel send error")]
    AudioChannelSendError,
    #[error("reqwest error")]
    ReqwestError(#[from] reqwest::Error),
    #[error("Audio cache dir error")]
    AudioCacheDirError,
    #[error("Zenoh error {0:?}")]
    ZenohError(#[from] zenoh::Error),
}
