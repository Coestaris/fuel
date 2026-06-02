use crate::id3v2::frame::{ID3v2ParseFrameError, parse_frame};
use crate::id3v2::header::{ID3v2ParseHeaderError, parse_header};
use crate::id3v2::payload::{parse_payload, ID3v2Frame, ID3v2ParsePayloadError};
use crate::id3v2::types::ID3v2FrameFlags;
use log::debug;
use std::io::{Read, Seek};
use thiserror::Error;

mod frame;
mod header;
mod payload;
mod types;

#[derive(Error, Debug)]
pub enum ID3v2Error {
    #[error("Failed to read data: {0}")]
    Read(#[from] std::io::Error),
    #[error("Failed to parse ID3v2 header: {0}")]
    ParseHeader(#[from] ID3v2ParseHeaderError),
    #[error("Failed to parse ID3v2 frame: {0}")]
    ParseFrame(#[from] ID3v2ParseFrameError),
    #[error("Failed to parse ID3v2 frame payload: {0}")]
    ParsePayload(#[from] ID3v2ParsePayloadError),
}

#[derive(Debug)]
pub struct ID3v2Tag {
    pub flags: ID3v2FrameFlags,
    pub frame: ID3v2Frame,
}

pub(crate) fn parse_id3v2<R: Read + Seek>(mut stream: R) -> Result<Vec<ID3v2Tag>, ID3v2Error> {
    let mut tags = Vec::new();

    let header = parse_header(&mut stream)?;
    debug!("Parsed ID3v2 header: {:?}", header);

    let end_pos = header.size.as_u32() as usize;
    while stream.stream_position()? < end_pos as u64 {
        let frame = parse_frame(&mut stream)?;
        debug!("Parsed ID3v2 frame: {:?}", frame);

        let payload = parse_payload(&mut stream, frame.frame_id, frame.size as usize)?;
        debug!("Parsed payload: {:?}", payload);

        tags.push(ID3v2Tag {
            flags: frame.flags,
            frame: payload,
        });
    }

    let padding = stream.stream_position()? - end_pos as u64 + size_of_val(&header) as u64;

    debug!("Position after reading frames: {}, padding: {}", stream.stream_position()?, padding);

    stream.seek(std::io::SeekFrom::Start(end_pos as u64 + padding))?;

    Ok(tags)
}
