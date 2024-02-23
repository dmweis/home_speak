use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use std::sync::mpsc::Sender;
use tokio::sync::mpsc::UnboundedSender as TokioSender;

use super::audio_player::{create_player, AudioPlayerCommand, Playable};
use crate::{error::HomeSpeakError, AUDIO_FILE_EXTENSION};

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct AudioMessage {
    pub data: String,
    pub format: String,
}

#[derive(Debug, Clone)]
pub struct AudioService {
    audio_sender: Sender<AudioPlayerCommand>,
    audio_data_broadcaster: Option<TokioSender<AudioMessage>>,
}

impl AudioService {
    pub fn new(audio_data_broadcaster: Option<TokioSender<AudioMessage>>) -> Result<Self> {
        let audio_sender = create_player();

        Ok(AudioService {
            audio_sender,
            audio_data_broadcaster,
        })
    }

    pub fn play(&self, mut data: Box<dyn Playable>) -> Result<()> {
        self.publish_audio_file(&mut data)?;
        self.audio_sender
            .send(AudioPlayerCommand::Play(data))
            .unwrap();
        Ok(())
    }

    pub fn restart_player(&self) -> Result<()> {
        self.audio_sender.send(AudioPlayerCommand::Restart).unwrap();
        Ok(())
    }

    fn publish_audio_file(&self, data: &mut Box<dyn Playable>) -> Result<()> {
        if let Some(sender) = self.audio_data_broadcaster.as_ref().cloned() {
            let payload = data.as_bytes()?;
            let base64_wav_file: String = general_purpose::STANDARD.encode(payload);
            let message = AudioMessage {
                data: base64_wav_file,
                format: AUDIO_FILE_EXTENSION.to_owned(),
            };
            sender
                .send(message)
                .map_err(|_| HomeSpeakError::AudioChannelSendError)?;
        }
        Ok(())
    }

    pub fn pause(&self) {
        self.audio_sender.send(AudioPlayerCommand::Pause).unwrap();
    }

    pub fn resume(&self) {
        self.audio_sender.send(AudioPlayerCommand::Resume).unwrap();
    }

    pub fn stop(&self) {
        self.audio_sender.send(AudioPlayerCommand::Stop).unwrap();
    }

    pub fn skip_one(&self) {
        self.audio_sender.send(AudioPlayerCommand::SkipOne).unwrap();
    }

    pub fn volume(&self, volume: f32) {
        self.audio_sender
            .send(AudioPlayerCommand::Volume(volume))
            .unwrap();
    }
}
