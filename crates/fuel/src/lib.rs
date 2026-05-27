use crate::id3v2::{ID3v2Error, ID3v2Tag, parse_id3v2};
use log::info;
use std::path::Path;
use thiserror::Error;

mod id3v2;

#[derive(Debug, Error)]
pub enum FuelError {
    #[error("Failed to read file: {0}")]
    ReadFile(#[from] std::io::Error),
    #[error("Failed to parse ID3v2 tag: {0}")]
    ID3v2(#[from] ID3v2Error),
}

#[derive(Debug)]
pub struct AudioFile {
    tags: Vec<ID3v2Tag>,
    sample_rate: u32,
    samples: Vec<f32>,
}

pub fn parse_file(path: &Path) -> Result<AudioFile, FuelError> {
    info!("Reading file: {}", path.display());
    let reader = std::fs::File::open(path)?;
    let (tags, audio_data) = parse_id3v2(reader)?;

    info!(
        "Parsed ID3v2 tags: {:?}, audio data length: {}",
        tags,
        audio_data.len()
    );

    Ok(AudioFile {
        tags,
        sample_rate: 44100,
        samples: Vec::new(),
    })
}
