use crate::error::{HomeSpeakError, Result};
use crate::speech_service::Playable;
use crate::AUDIO_FILE_EXTENSION;
use std::fs::{self, File};
use std::io::prelude::*;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct AudioCache {
    cache_dir_path: Option<String>,
}

impl AudioCache {
    pub fn new(cache_dir_path: String) -> Result<AudioCache> {
        let path = Path::new(&cache_dir_path);
        fs::create_dir_all(path)?;
        if !path.exists() {
            return Err(HomeSpeakError::CacheDirPathNotFound);
        }
        Ok(AudioCache {
            cache_dir_path: Some(cache_dir_path),
        })
    }

    pub fn new_without_cache() -> AudioCache {
        AudioCache {
            cache_dir_path: None,
        }
    }

    pub fn get(&self, key: &str) -> Option<Box<dyn Playable>> {
        let cache_dir_path = match &self.cache_dir_path {
            Some(path) => path,
            None => return None,
        };
        let path = Path::new(cache_dir_path);
        let file_path = path.join(format!("{}.{}", key, AUDIO_FILE_EXTENSION));
        if let Ok(file) = File::open(file_path) {
            Some(Box::new(file))
        } else {
            None
        }
    }

    pub fn set(&self, key: &str, contents: Vec<u8>) -> Result<()> {
        let cache_dir_path = match &self.cache_dir_path {
            Some(path) => path,
            None => return Ok(()),
        };
        let path = Path::new(cache_dir_path);
        let file_path = path.join(format!("{}.{}", key, AUDIO_FILE_EXTENSION));
        let mut file = File::create(file_path)?;
        file.write_all(&contents)?;
        file.flush()?;
        Ok(())
    }
}
