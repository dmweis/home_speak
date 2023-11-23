use crate::error::{HomeSpeakError, Result};
use log::*;
use rodio::cpal::traits::{DeviceTrait, HostTrait};
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
    Restart,
}

/// Select the first audio output device that contains "CARD=Device" in its name
/// This is a hack to select the USB audio device on the raspberry pi
/// Based on https://github.com/RustAudio/rodio/blob/4973f330e07be8480c35f145c9da84dc60e2184c/src/stream.rs#L57
fn select_output_device() -> anyhow::Result<(rodio::OutputStream, rodio::OutputStreamHandle)> {
    let host_ids = rodio::cpal::available_hosts();
    for host_id in host_ids {
        let host = rodio::cpal::host_from_id(host_id).unwrap();
        info!("Found audio host {}", host_id.name());
        let output_devices = host.devices().unwrap();
        for device in output_devices {
            let device_name = device.name().unwrap_or_default();
            if device_name.contains("CARD=Device") {
                let default_stream = rodio::OutputStream::try_from_device(&device);

                return Ok(default_stream.or_else(|original_err| {
                    // default device didn't work, try other ones
                    let mut devices = match rodio::cpal::default_host().output_devices() {
                        Ok(d) => d,
                        Err(_) => return Err(original_err),
                    };

                    devices
                        .find_map(|d| rodio::OutputStream::try_from_device(&d).ok())
                        .ok_or(original_err)
                })?);
            }
        }
    }
    anyhow::bail!("No audio output device found");
}

fn audio_player_loop(receiver: &Receiver<AudioPlayerCommand>) -> anyhow::Result<bool> {
    // let (_output_stream, output_stream_handle) = rodio::OutputStream::try_default()
    //     .map_err(|_| HomeSpeakError::FailedToCreateAnOutputStream)?;

    let (_output_stream, output_stream_handle) = select_output_device()?;

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
            AudioPlayerCommand::Restart => {
                info!("Restarting audio player");
                return Ok(false);
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
    thread::spawn(move || {
        // This may miss on sender being dead. But if sender is dead we have bigger issues
        loop {
            match audio_player_loop(&receiver) {
                Err(err) => {
                    error!("Audio player loop failed with {}", err);
                }
                Ok(terminate) => {
                    if terminate {
                        break;
                    }
                }
            }
        }
    });
    sender
}
