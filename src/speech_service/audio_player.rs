use crate::error::{HomeSpeakError, Result};
use log::*;
use std::io::Seek;
use std::sync::mpsc::Receiver;
use std::{
    fs::File,
    io::{Cursor, Read},
    sync::mpsc::{channel, Sender},
    thread,
};

pub trait Playable: std::io::Read + std::io::Seek + Send + Sync {
    fn as_bytes(&mut self) -> Result<Vec<u8>>;
}

impl Playable for Cursor<Vec<u8>> {
    fn as_bytes(&mut self) -> Result<Vec<u8>> {
        Ok(self.get_ref().clone())
    }
}
impl Playable for File {
    fn as_bytes(&mut self) -> Result<Vec<u8>> {
        let mut buffer = vec![];
        self.read_to_end(&mut buffer)?;
        self.seek(std::io::SeekFrom::Start(0))?;
        Ok(buffer)
    }
}

pub enum AudioPlayerCommand {
    Play(Box<dyn Playable>),
    Pause,
    Resume,
    Stop,
    Volume(f32),
}

fn audio_player_loop(
    receiver: &Receiver<AudioPlayerCommand>,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let (_output_stream, output_stream_handle) = rodio::OutputStream::try_default()
        .map_err(|_| HomeSpeakError::FailedToCreateAnOutputStream)?;
    let sink = rodio::Sink::try_new(&output_stream_handle)
        .map_err(|_| HomeSpeakError::FailedToCreateASink)?;
    loop {
        let command = receiver.recv()?;
        match command {
            AudioPlayerCommand::Play(sound) => {
                sink.append(
                    rodio::Decoder::new(sound)
                        .map_err(|_| HomeSpeakError::FailedToDecodeAudioFile)?,
                );
            }
            AudioPlayerCommand::Pause => {
                info!("Pausing audio");
                sink.pause()
            }
            AudioPlayerCommand::Resume => {
                info!("Resuming audio");
                sink.play()
            }
            AudioPlayerCommand::Stop => {
                info!("Stopping audio");
                warn!("Ignoring stop because it destroys the sink");
                // sink.stop()
            }
            AudioPlayerCommand::Volume(volume) => {
                info!("Settings volume to {}", volume);
                sink.set_volume(volume)
            }
        }
    }
}

pub fn create_player() -> Sender<AudioPlayerCommand> {
    let (sender, receiver) = channel();
    thread::spawn(move || loop {
        // This may miss on sender being dead. But if sender is dead we have bigger issues
        loop {
            if let Err(e) = audio_player_loop(&receiver) {
                error!("Audio player loop failed with {}", e);
            }
        }
    });
    sender
}
