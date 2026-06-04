use crate::bite::{Biter, BiterError, DEFAULT_BUF_SIZE};
use crate::header::{MPEGHeader, MPEGVersion, MpegParseHeaderError, parse_header};
use crate::id3v2::{ID3v2Error, ID3v2Tag, parse_id3v2};
use crate::mpeg1::{MPEG1DecoderNewError, mpeg1_decoder_new};
use log::{debug, info, warn};
use std::error::Error;
use std::io::{Read, Seek};
use std::path::Path;
use thiserror::Error;

mod bite;
pub mod header;
mod id3v2;
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
    #[error("Failed to create MPEG1 decoder: {0}")]
    FailedToCreateMPEG1Decoder(#[from] MPEG1DecoderNewError),
    #[error("Failed decode error: {0}")]
    FrameDecodeError(#[from] Box<dyn Error>),
}

#[derive(Debug)]
pub struct Frame {
    sample_rate: u32,
    samples: [f32; 511],
    used: usize,
}

#[derive(Debug)]
pub struct Audio {
    tags: Vec<ID3v2Tag>,
    frames: Vec<Frame>,
}

pub(crate) trait Decoder<R: Read, const BUF_SIZE: usize> {
    fn decode_samples(
        &mut self,
        header: &MPEGHeader,
        biter: &mut Biter<R, BUF_SIZE>,
    ) -> Result<Frame, Box<dyn Error>>;
}

pub fn decode_file(path: &Path) -> Result<Audio, FuelError> {
    info!("Reading file: {}", path.display());
    decode_stream(std::fs::File::open(path)?)
}

pub fn decode_stream<R: Read + Seek>(mut stream: R) -> Result<Audio, FuelError> {
    let tags = parse_id3v2(&mut stream)?;
    let mut biter: Biter<&mut _, { DEFAULT_BUF_SIZE }> = Biter::new(&mut stream);

    let mut decoder = None;
    let mut frames = vec![];

    loop {
        let header = match parse_header(&mut biter) {
            Err(MpegParseHeaderError::BiterError(BiterError::EOF)) => break,
            h => h?,
        };
        debug!("Header {:#?}", header);

        if let None = &decoder {
            decoder = Some(match header.version {
                MPEGVersion::MPEG1 => mpeg1_decoder_new(&header),
                MPEGVersion::MPEG2 => Err(FuelError::UnsupportedVersion(header.version))?,
                MPEGVersion::MPEG25 => Err(FuelError::UnsupportedVersion(header.version))?,
            }?)
        };

        frames.push(
            decoder
                .as_mut()
                .unwrap()
                .decode_samples(&header, &mut biter)?,
        );

        if frames.len() == 2 {
            warn!("Stopping for debug reasons");
            break;
        }
    }

    Ok(Audio { tags, frames })
}
