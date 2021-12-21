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
    // TODO: Propagate errors from google_tts
    #[error("google tts failed to synthesize")]
    GoogleTtsError,
}
