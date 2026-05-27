use crate::id3v2::types::{ID3v2FrameFlags, ID3v2FrameHeader, ID3v2FrameID};
use bitflags::Flags;
use log::error;
use std::io::Read;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ID3v2ParseFrameError {
    #[error("Failed to read data: {0}")]
    Read(#[from] std::io::Error),
    #[error("Invalid bytes in frame ID: {0:?}")]
    InvalidFrameID(ID3v2FrameID),
    #[error("Unknown flags set in ID3v2 frame header: {0:?}")]
    UnknownHeaderFlags(ID3v2FrameFlags),
    #[error("Not implemented flags set in ID3v2 frame header: {0:?}")]
    NotImplementedFlags(ID3v2FrameFlags),
}

pub(crate) fn parse_frame<R: Read>(
    stream: &mut R,
) -> Result<ID3v2FrameHeader, ID3v2ParseFrameError> {
    // SAFETY: Any of the fields contained in the header are valid,
    // and the header is packed, so there are no padding bytes to worry about.
    let header = unsafe {
        let mut header: ID3v2FrameHeader = std::mem::zeroed();
        stream.read_exact(std::slice::from_raw_parts_mut(
            &mut header as *mut ID3v2FrameHeader as *mut u8,
            size_of::<ID3v2FrameHeader>(),
        ))?;

        // Size in big-endian
        header.size = header.size.swap_bytes();
        header
    };

    if let None = header.frame_id.as_str() {
        return Err(ID3v2ParseFrameError::InvalidFrameID(header.frame_id));
    }

    // Copy to avoid unaligned access
    let flags = header.flags;
    if flags.contains_unknown_bits() {
        return Err(ID3v2ParseFrameError::UnknownHeaderFlags(flags));
    }

    if !flags.is_empty() {
        return Err(ID3v2ParseFrameError::NotImplementedFlags(flags));
    }

    Ok(header)
}
