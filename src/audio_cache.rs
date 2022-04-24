use crate::error::{HomeSpeakError, Result};
use crate::speech_service::PlayAble;
use std::fs::{self, File};
use std::io::prelude::*;
use std::path::Path;

pub(crate) struct AudioCache {
    cache_dir_path: String,
}

impl AudioCache {
    pub(crate) fn new(cache_dir_path: String) -> Result<AudioCache> {
        let path = Path::new(&cache_dir_path);
        fs::create_dir_all(path)?;
        if !path.exists() {
            return Err(HomeSpeakError::CacheDirPathNotFound);
        }
        Ok(AudioCache { cache_dir_path })
    }

    pub(crate) fn get(&self, key: &str) -> Option<Box<dyn PlayAble>> {
        let path = Path::new(&self.cache_dir_path);
        let file_path = path.join(format!("{}.mp3", key));
        if let Ok(file) = File::open(file_path) {
            Some(Box::new(file))
        } else {
            None
        }
    }

    pub(crate) fn set(&self, key: &str, contents: Vec<u8>) -> Result<()> {
        let path = Path::new(&self.cache_dir_path);
        let file_path = path.join(format!("{}.mp3", key));
        let mut file = File::create(file_path)?;
        file.write_all(&contents)?;
        file.flush()?;
        Ok(())
    }
}
