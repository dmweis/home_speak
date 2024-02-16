use super::AudioService;
use crate::error::{HomeSpeakError, Result};
use rand::seq::SliceRandom;
use std::fs::{self, File};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct AudioRepository {
    dir_path: PathBuf,
    audio_service: AudioService,
}

impl AudioRepository {
    pub fn new(dir_path: &Path, audio_service: AudioService) -> Result<Self> {
        let path = Path::new(&dir_path);
        fs::create_dir_all(path)?;
        if !path.exists() {
            return Err(HomeSpeakError::AudioCacheDirError);
        }
        Ok(Self {
            dir_path: dir_path.to_owned(),
            audio_service,
        })
    }

    pub fn play_file(&self, sound_name: &str) -> anyhow::Result<bool> {
        let file_path = self.dir_path.join(sound_name);
        if let Ok(file) = File::open(file_path) {
            self.audio_service.play(Box::new(file))?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn random_file_from_dir(&self, subdirectory: &str) -> anyhow::Result<bool> {
        // This is a pretty complicated way to do this
        // but oh well... it's fast enough for now

        let full_path = self.dir_path.join(subdirectory);

        if let Ok(paths) = std::fs::read_dir(full_path) {
            let files: Vec<_> = paths
                .filter_map(|path| path.ok())
                .filter(|entry| {
                    entry
                        .file_type()
                        .map(|file_type| file_type.is_file())
                        .unwrap_or(false)
                })
                .map(|file_entry| file_entry.path())
                .collect();
            let mut rng = rand::thread_rng();

            let audio_file = files
                .choose(&mut rng)
                .and_then(|path| File::open(path).ok())
                .map(Box::new);

            if let Some(audio_file) = audio_file {
                self.audio_service.play(audio_file)?;
                Ok(true)
            } else {
                Ok(false)
            }
        } else {
            Ok(false)
        }
    }

    pub fn random_file_recursive(&self) -> anyhow::Result<()> {
        let walker = walkdir::WalkDir::new(&self.dir_path).into_iter();

        let files = walker
            // ignore errors
            .filter_map(|entry| entry.ok())
            // take only files
            .filter(|entry| entry.file_type().is_file())
            // turn to paths
            .map(|file_entry| file_entry.path().to_path_buf())
            // ignore astromech because those files are boring
            .filter(|path| !path.to_string_lossy().contains("astromech"))
            .collect::<Vec<_>>();

        let mut rng = rand::thread_rng();

        let audio_file = files
            .choose(&mut rng)
            .and_then(|path| File::open(path).ok())
            .map(Box::new);

        if let Some(audio_file) = audio_file {
            self.audio_service.play(audio_file)?;
        }
        Ok(())
    }
}
