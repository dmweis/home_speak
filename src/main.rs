use clap::Clap;
use std::env;
use google_tts;
use rodio::{self, DeviceTrait};
use std::io::Cursor;
use std::io::BufReader;


#[derive(Clap)]
#[clap(
    version = "0.1.0",
    author = "David M. Weis <dweis7@gmail.com>",
    about = "CLI tool for playing text to speech commands using Google text to speech cloud API"
)]
struct Opts {
    #[clap(
        short = "m",
        long = "message",
        about = "text"
    )]
    message: String,
}

fn play_sound(data: Vec<u8>) {
    let buffer = Cursor::new(data);
    let output_device = rodio::default_output_device().expect("Failed getting default device");
    if output_device.default_output_format().is_ok() {
        let output_sink = rodio::Sink::new(&output_device);
        output_sink.append(rodio::Decoder::new(BufReader::new(buffer)).expect("Failed accessing output device"));
        output_sink.sleep_until_end();
    } else {
        eprintln!("Device doesn't support output");
    }
}

fn main() {
    let google_api_key = env::var("GOOGLE_API_KEY").expect("Please set GOOGLE_API_KEY");
    let opts: Opts = Opts::parse();

    let client = google_tts::GoogleTtsClient::new(google_api_key);
    let data = client.synthesize(
        google_tts::TextInput::with_text(opts.message.to_owned()),
        google_tts::VoiceProps::default_english_female_wavenet(),
        google_tts::AudioConfig::default_with_encoding(google_tts::AudioEncoding::Mp3)
    ).unwrap();

    play_sound(data.as_byte_stream().unwrap());
}
