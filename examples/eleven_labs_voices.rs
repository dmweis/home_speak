use clap::Parser;
use home_speak::eleven_labs_client;

#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Opts {
    #[clap(long)]
    key: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let opts = Opts::from_args();

    let eleven_labs_client = eleven_labs_client::ElevenLabsTtsClient::new(opts.key);
    let voices = eleven_labs_client.voices().await?;

    println!("{:}", serde_json::to_string_pretty(&voices)?);

    Ok(())
}
