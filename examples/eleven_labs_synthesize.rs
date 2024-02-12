use clap::Parser;
use home_speak::{
    configuration::get_configuration,
    eleven_labs_client::{self, VoiceSettings, DEFAULT_MODEL},
    speech_service::DEFAULT_ELEVEN_LABS_VOICE_ID,
};
use secrecy::ExposeSecret;

#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    #[clap(long)]
    key: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let app_config = get_configuration(None)?;

    let key = if let Some(key) = args.key {
        key
    } else {
        app_config
            .tts_service_config
            .eleven_labs_api_key
            .expose_secret()
            .to_owned()
    };

    let eleven_labs_client = eleven_labs_client::ElevenLabsTtsClient::new(key);
    let data = eleven_labs_client
        .tts(
            "Hello world",
            DEFAULT_ELEVEN_LABS_VOICE_ID,
            Some(VoiceSettings::default()),
            DEFAULT_MODEL,
        )
        .await?;

    println!("data size {:?} bytes", data.len());

    Ok(())
}
