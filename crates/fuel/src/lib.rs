use crate::id3v2::{ID3v2Error, ID3v2Tag, parse_id3v2};
use crate::mpeg::{MpegError, parse_mpeg};
use log::{debug, info};
use std::path::Path;
use thiserror::Error;

mod id3v2;
mod bite;
mod mpeg;

#[derive(Debug, Error)]
pub enum FuelError {
    #[error("Failed to read file: {0}")]
    ReadFile(#[from] std::io::Error),
    #[error("Failed to parse ID3v2 tag: {0}")]
    ID3v2(#[from] ID3v2Error),
    #[error("Failed to parse MPEG: {0}")]
    MPEG(#[from] MpegError),
}

#[derive(Debug)]
pub struct AudioFile {
    tags: Vec<ID3v2Tag>,
    sample_rate: u32,
    samples: Vec<f32>,
}

pub fn parse_file(path: &Path) -> Result<AudioFile, FuelError> {
    info!("Reading file: {}", path.display());
    let mut reader = std::fs::File::open(path)?;
    let tags = parse_id3v2(&mut reader)?;
    let samples = parse_mpeg(&mut reader)?;

    Ok(AudioFile {
        tags,
        sample_rate: 44100,
        samples: samples,
    })
}
