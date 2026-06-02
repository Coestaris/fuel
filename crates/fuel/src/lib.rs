use crate::id3v2::{ID3v2Error, ID3v2Tag, parse_id3v2};
use log::{debug, info};
use std::path::Path;
use thiserror::Error;
use crate::bite::{Biter, DEFAULT_BUF_SIZE};
use crate::header::{parse_header, MPEGVersion, MpegParseHeaderError};
use crate::mpeg1::{parse_mpeg1, MPEG1ParseError};

mod id3v2;
mod bite;
pub mod header;
mod mpeg1;

#[derive(Debug, Error)]
pub enum FuelError {
    #[error("Failed to read file: {0}")]
    ReadFile(#[from] std::io::Error),
    #[error("Failed to parse ID3v2 tag: {0}")]
    ID3v2(#[from] ID3v2Error),
    #[error("Failed to parse header: {0}")]
    ParseHeader(#[from] MpegParseHeaderError),
    #[error("Unsupported MPEG version: {0}")]
    UnsupportedVersion(MPEGVersion),
    #[error(transparent)]
    MPEG1Error(#[from] MPEG1ParseError),
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

    let mut biter: Biter<&mut _, { DEFAULT_BUF_SIZE }> = Biter::new(&mut reader);
    let header = parse_header(&mut biter)?;
    debug!("Header {:#?}", header);

    let samples = match header.version {
        MPEGVersion::MPEG1 => parse_mpeg1(&header, &mut biter),
        MPEGVersion::MPEG2 => Err(FuelError::UnsupportedVersion(header.version))?,
        MPEGVersion::MPEG25 => Err(FuelError::UnsupportedVersion(header.version))?,
    }?;
    debug!("Samples {:#?}", samples);

    Ok(AudioFile {
        tags,
        sample_rate: 44100,
        samples,
    })
}
