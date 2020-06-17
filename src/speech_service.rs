use google_tts;
use std::io::Cursor;
use std::io::BufReader;
use rodio::{self, DeviceTrait};
use log::*;

pub struct SpeechService {
    speech_client: google_tts::GoogleTtsClient,
    output_sink: rodio::Sink,
}

impl SpeechService {
    pub fn new(google_api_key: String) -> Result<SpeechService, Box<dyn std::error::Error>> {
        let client = google_tts::GoogleTtsClient::new(google_api_key);

        let output_device = rodio::default_output_device().ok_or("Failed to get default output device")?;
        info!("Started SpeechService with {}", output_device.name()?);
        let output_sink = rodio::Sink::new(&output_device);

        Ok(SpeechService {
            speech_client: client,
            output_sink,
        })
    }

    pub fn say(&self, text: String) -> Result<(), Box<dyn std::error::Error>> {
        let data = self.speech_client.synthesize(
            google_tts::TextInput::with_text(text),
            google_tts::VoiceProps::default_english_female_wavenet(),
            google_tts::AudioConfig::default_with_encoding(google_tts::AudioEncoding::Mp3)
        )?;

        let buffer = Cursor::new(data.as_byte_stream()?);
        self.output_sink.append(rodio::Decoder::new(BufReader::new(buffer))?);
        Ok(())
    }
}
