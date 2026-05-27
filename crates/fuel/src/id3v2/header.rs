use crate::id3v2::types::{ID3V2_MAGIC, ID3v2Header, ID3v2HeaderFlags, ID3v2Magic};
use bitflags::Flags;
use std::io::Read;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ID3v2ParseHeaderError {
    #[error("Failed to read data: {0}")]
    Read(#[from] std::io::Error),
    #[error("Invalid ID3v2 tag, expected magic 'ID3' but got {0:?}")]
    InvalidMagic(ID3v2Magic),
    #[error("Unsupported ID3v2 version: {}.{}", .0, .1)]
    InvalidVersion(u8, u8),
    #[error("Unknown header flags set in ID3v2 header: {0:?}")]
    UnknownHeaderFlags(ID3v2HeaderFlags),
    #[error("Not implemented flags set in ID3v2 header: {0:?}")]
    NotImplementedFlags(ID3v2HeaderFlags),
}

pub(crate) fn parse_header<R: Read>(stream: &mut R) -> Result<ID3v2Header, ID3v2ParseHeaderError> {
    // SAFETY: Any of the fields contained in the header are valid,
    // and the header is packed, so there are no padding bytes to worry about.
    let header = unsafe {
        let mut header: ID3v2Header = std::mem::zeroed();
        stream.read_exact(std::slice::from_raw_parts_mut(
            &mut header as *mut ID3v2Header as *mut u8,
            size_of::<ID3v2Header>(),
        ))?;

        header
    };

    if header.magic != ID3V2_MAGIC {
        return Err(ID3v2ParseHeaderError::InvalidMagic(header.magic));
    }

    if header.flags.contains_unknown_bits() {
        return Err(ID3v2ParseHeaderError::UnknownHeaderFlags(header.flags));
    }

    if header.major_version == 0xFF || header.minor_version == 0xFF {
        return Err(ID3v2ParseHeaderError::InvalidVersion(
            header.major_version,
            header.minor_version,
        ));
    }

    if !header.flags.is_empty() {
        return Err(ID3v2ParseHeaderError::NotImplementedFlags(header.flags));
    }

    Ok(header)
}
