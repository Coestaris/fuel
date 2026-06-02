use std::io::{Read, Seek};
use log::{debug, info};
use thiserror::Error;
use crate::bite::{Biter, DEFAULT_BUF_SIZE};
use crate::mpeg::header::{parse_header, MpegParseHeaderError};

pub mod header;

#[derive(Debug, Error)]
pub enum MpegError {
    #[error("Failed to read data: {0}")]
    Read(#[from] std::io::Error),
    #[error("Failed to parse header: {0}")]
    ParseHeader(#[from] MpegParseHeaderError)
}

pub(crate) fn parse_mpeg<R: Read + Seek>(mut stream: R) -> Result<Vec<f32>, MpegError> {
    let mut biter: Biter<&mut _, { DEFAULT_BUF_SIZE }> = Biter::new(&mut stream);
    let header = parse_header(&mut biter)?;
    debug!("Parsed {:#?}", header);

    Ok(vec![])
}