use bitflags::bitflags;
use std::fmt::{Debug, Write};

bitflags! {
    #[repr(C)]
    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    pub struct ID3v2HeaderFlags: u8 {
        // indicates whether unsynchronisation is used
        const UNSYNCHRONIZATION = 0b1000_0000;
        // indicates whether the header is followed by an extended header
        const EXTENDED_HEADER = 0b0100_0000;
        // should be used as an 'experimental indicator'.
        // This flag should always be set when the tag is in an experimental stage.
        const EXPERIMENTAL_INDICATOR = 0b0010_0000;
    }
}

// Tag size is encoded with four bytes where the most significant bit (bit 7)
// is set to zero in every byte, making a total of 28 bits. The zeroed bits are ignored,
// so a 257 bytes long tag is represented as $00 00 02 01.
//
// Only 28 bits (representing up to 256MB) are used in the size description
// to avoid the in
// troduction of 'false syncsignals'.
#[repr(C)]
#[repr(packed)]
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct ID3v2Size(u32);

#[repr(C)]
#[repr(packed)]
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct ID3v2Magic([u8; 3]);

// The ID3v2 tag header, which should be the first information in the file
#[repr(C)]
#[repr(packed)]
#[derive(Debug, Copy, Clone)]
pub(crate) struct ID3v2Header {
    // the tag are always "ID3" to indicate that this is an ID3v2 tag
    pub magic: ID3v2Magic,

    // Version and revision will never be $FF.
    pub major_version: u8,
    pub minor_version: u8,

    // All the other flags should be cleared. If one of these undefined flags
    // are set that might mean that the tag is not readable for a parser that does
    // not know the flags function.
    pub flags: ID3v2HeaderFlags,

    // The ID3v2 tag size is the size of the complete tag after unsychronisation,
    // including padding, excluding the header but not excluding the extended header
    // (total tag size - 10).
    pub size: ID3v2Size,
}

bitflags! {
    #[repr(C)]
    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    pub struct ID3v2ExtendedFlags: u16 {
        // If this flag is set four bytes of CRC-32 data is appended to the extended header.
        // The CRC should be calculated before unsynchronisation on the data between the extended
        // header and the padding, i.e. the frames and only the frames.
        const CRC_DATA_PRESENT = 0b1000_0000_0000_0000;
    }
}

// The extended header contains information that is not vital to the correct
// parsing of the tag information, hence the extended header is optional.
// The extended header is considered separate from the header proper,
// and as such is subject to unsynchronisation
#[repr(C)]
#[repr(packed)]
#[derive(Debug, Copy, Clone)]
#[allow(unused)]
pub(crate) struct ID3v2ExtendedHeader {
    // Where the 'Extended header size', currently 6 or 10 bytes, excludes itself.
    // big-endian
    pub extended_header_size: u32,

    // The extended flags are a secondary flag set which
    // describes further attributes of the tag.
    pub extended_flags: ID3v2ExtendedFlags,

    // is simply the total tag size excluding the frames and the headers,
    // in other words the padding.
    // big-endian
    pub size_of_padding: u32,
}

// The frame ID made out of the characters capital A-Z and 0-9.
// Identifiers beginning with "X", "Y" and "Z" are for experimental use and free for
// everyone to use, without the need to set the experimental bit in the tag header.
// Have in mind that someone else might have used the same identifier as you.
// All other identifiers are either used or reserved for future use.
#[repr(C)]
#[repr(packed)]
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct ID3v2FrameID([u8; 4]);

bitflags! {
    #[repr(C)]
    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    pub struct ID3v2FrameFlags: u16 {
        // If this flag is set the frame should be preserved if the tag is altered,
        // for example when adding or removing frames, or when changing the size of the tag.
        const TAG_ALTER_PRESERVATION = 0b1000_0000_0000_0000;

        // If this flag is set the frame should be preserved if the file, excluding the tag, is altered,
        // for example when audio is being cut or merged, or when a different audio encoding is used.
        const FILE_ALTER_PRESERVATION = 0b0100_0000_0000_0000;

        // If this flag is set the frame should be read only. A tag editor should not allow a user to edit
        // the contents of a read only frame. A tag editor should not allow a user to remove a read only frame.
        const READ_ONLY = 0b0010_0000_0000_0000;

        // This flag indicates whether or not the frame is compressed.
        const COMPRESSION = 0b0000_0000_1000_0000;

        // This flag indicates wether or not the frame is enrypted. If set one byte indicating with
        // which method it was encrypted will be appended to the frame header.
        const ENCRYPTED = 0b0000_0000_0100_0000;

        // This flag indicates whether or not the frame contains a group identifier.
        // If set one byte containing a group identifier will be appended to the frame header.
        const GROUPING_IDENTITY = 0b0000_0000_0010_0000;
    }
}

#[repr(C)]
#[repr(packed)]
#[derive(Debug, Copy, Clone)]
pub(crate) struct ID3v2FrameHeader {
    pub frame_id: ID3v2FrameID,

    // A tag must contain at least one frame.
    // A frame must be at least 1 byte big, excluding the header.
    // If nothing else is said a string is represented as ISO-8859-1 characters in the range
    // $20 - $FF. Such strings are represented as <text string>, or <full text string>
    // if newlines are allowed, in the frame descriptions.
    // All Unicode strings use 16-bit unicode 2.0 (ISO/IEC 10646-1:1993, UCS-2).
    // Unicode strings must begin with the Unicode BOM ($FF FE or $FE FF)
    // to identify the byte order.
    // big-endian
    pub size: u32,

    pub flags: ID3v2FrameFlags,
}

pub(crate) const ID3V2_MAGIC: ID3v2Magic = ID3v2Magic(*b"ID3");

impl ID3v2Size {
    pub fn as_u32(&self) -> u32 {
        // Data in big endian
        (self.0 & 0x7F000000) >> 24
            | (self.0 & 0x007F0000) >> 16
            | (self.0 & 0x00007F00) >> 8
            | (self.0 & 0x0000007F)
    }
}

mod tests {
    use crate::id3v2::types::*;

    #[test]
    fn size_conversion() {
        assert_eq!(ID3v2Size(40).as_u32(), 40);
        assert_eq!(ID3v2Size(0x00000201).as_u32(), 257);
    }
}

impl Debug for ID3v2Size {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.as_u32(), f)
    }
}

impl Debug for ID3v2Magic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::str::from_utf8(&self.0)
            .map(|s| f.write_str(s))
            .unwrap_or_else(|_| self.0.fmt(f))
    }
}

impl ID3v2FrameID {
    pub fn as_str(&self) -> Option<&str> {
        // Can contain only uppercase latin in UTF-8
        let r = std::str::from_utf8(&self.0).ok()?;
        if r.chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        {
            Some(r)
        } else {
            None
        }
    }
}

impl Debug for ID3v2FrameID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_str()
            .map(|s| f.write_str(s))
            .unwrap_or_else(|| self.0.fmt(f))
    }
}
