use crate::bite::{Biter, DEFAULT_BUF_SIZE};
use crate::mpeg::header::{MpegParseHeaderError, parse_header};
use crate::mpeg::main_data::{MpegParseMainDataError, parse_main_data};
use crate::mpeg::side_info::{MpegParseSideInfoError, parse_side_info};
use log::debug;
use std::io::{Read, Seek};
use thiserror::Error;

pub mod header;
mod main_data;
mod side_info;

#[derive(Debug, Error)]
pub enum MpegError {
    #[error("Failed to read data: {0}")]
    Read(#[from] std::io::Error),
    #[error("Failed to parse header: {0}")]
    ParseHeader(#[from] MpegParseHeaderError),
    #[error("Failed to parse audio data: {0}")]
    ParseAudioData(#[from] MpegParseSideInfoError),
    #[error("Failed to parse main data: {0}")]
    ParseMainData(#[from] MpegParseMainDataError),
}

pub(crate) fn parse_mpeg<R: Read + Seek>(mut stream: R) -> Result<Vec<f32>, MpegError> {
    let mut biter: Biter<&mut _, { DEFAULT_BUF_SIZE }> = Biter::new(&mut stream);
    let header = parse_header(&mut biter)?;
    debug!("Header {:#?}", header);

    debug!("after header bit pos = {}", biter.bits_read());
    let (side_info, main_data_begin) = parse_side_info(&header, &mut biter)?;
    debug!("after side info bit pos = {}", biter.bits_read());

    debug!("Main data begin {:#?}", main_data_begin);
    debug!("Size info {:#?}", side_info);

    let main_data = parse_main_data(&header, &side_info, main_data_begin, &mut biter)?;
    debug!("after main data bit pos = {}", biter.bits_read());
    debug!("Main data {:#?}", main_data);

    Ok(vec![])
}
