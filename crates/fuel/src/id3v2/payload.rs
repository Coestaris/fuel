use crate::id3v2::types::ID3v2FrameID;
use lazy_static::lazy_static;
use log::error;
use std::collections::HashMap;
use std::io::Read;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ID3v2ParsePayloadError {
    #[error("Failed to read payload data: {0}")]
    Read(#[from] std::io::Error),
    #[error("Unknown ID3v2 frame ID: {0:?}")]
    UnknownTag(ID3v2FrameID),
    #[error("Unknown encoding type: {0}")]
    UnknownEncodingType(u8),
    #[error("Failed to parse string with encoding {0}: {1:?}")]
    StringParseError(&'static str, Vec<u8>),
}

type FFOOFO = HashMap<
    &'static str,
    Box<dyn Fn(&[u8]) -> Result<ID3v2Frame, ID3v2ParsePayloadError> + Send + Sync>,
>;

macro_rules! impl_id3v2_frame {
    ( $( { $id:literal, $name:ident, ($ty:ty), $parser:ident } ),* $(,)? ) => {
        #[derive(Debug)]
        pub enum ID3v2Frame {
            Dummy,
            $(
                $name($ty),
            )*

        }

        lazy_static! {
            pub static ref ID3V2_FRAME_PARSERS: FFOOFO = {
                let mut m: FFOOFO = HashMap::new();
                $(
                    m.insert($id, Box::new(|data| {
                        let parsed = $parser(data)?;
                        Ok(ID3v2Frame::$name(parsed))
                    }));
                )*
                m
            };
        }
    }
}

fn parse_iso_8859_1(data: &[u8]) -> Result<String, ID3v2ParsePayloadError> {
    unimplemented!()
}

fn parse_utf16(data: &[u8]) -> Result<String, ID3v2ParsePayloadError> {
    unimplemented!()
}

fn parse_utf16be(data: &[u8]) -> Result<String, ID3v2ParsePayloadError> {
    unimplemented!()
}

fn parse_utf8(data: &[u8]) -> Result<String, ID3v2ParsePayloadError> {
    let vec = data.to_vec();
    // Get rid of the null terminator if it exists
    let vec = if vec.last() == Some(&0) {
        vec[..vec.len() - 1].to_vec()
    } else {
        vec
    };

    String::from_utf8(vec)
        .map_err(|_| ID3v2ParsePayloadError::StringParseError("UTF-8", data.to_vec()))
}

fn string_parser(arg: &[u8]) -> Result<String, ID3v2ParsePayloadError> {
    lazy_static! {
        pub static ref ENCODINGS: HashMap<u8, Box<dyn Fn(&[u8]) -> Result<String, ID3v2ParsePayloadError> + Send + Sync>> = {
            let mut m: HashMap<
                u8,
                Box<dyn Fn(&[u8]) -> Result<String, ID3v2ParsePayloadError> + Send + Sync>,
            > = HashMap::new();
            m.insert(0, Box::new(parse_iso_8859_1));
            m.insert(1, Box::new(parse_utf16));
            m.insert(2, Box::new(parse_utf16be));
            m.insert(3, Box::new(parse_utf8));
            m
        };
    }

    let encoding = arg[0];
    if ENCODINGS.contains_key(&encoding) {
        ENCODINGS[&encoding](&arg[1..])
    } else {
        Err(ID3v2ParsePayloadError::UnknownEncodingType(encoding))
    }
}

fn i64_parser(arg: &[u8]) -> Result<i64, ID3v2ParsePayloadError> {
    let s = string_parser(arg)?;
    s.parse::<i64>()
        .map_err(|_| ID3v2ParsePayloadError::StringParseError("integer", arg.to_vec()))
}

impl_id3v2_frame! (
    { "TALB", Album, (String), string_parser },
    { "TBPM", BPM, (i64), i64_parser },
    { "TCOM", Composer, (String), string_parser },
    { "TCON", ContentType, (String), string_parser },
    { "TCOP", CopyrightNessage, (String), string_parser },
    { "TDAT", Date, (String), string_parser },
    { "TDLY", PlaylistDelay, (String), string_parser },
    { "TENC", EncodedBy, (String), string_parser },
    { "TEXT", Lyricist, (String), string_parser },
    { "TFLT", FileType, (String), string_parser },
    { "TIME", Time, (String), string_parser },
    { "TIT1", ContentGroupDescription, (String), string_parser },
    { "TIT2", Title, (String), string_parser },
    { "TIT3", Subtitle, (String), string_parser },
    { "TKEY", InitialKey, (String), string_parser },
    { "TLAN", Language, (String), string_parser },
    { "TLEN", Length, (String), string_parser },
    { "TMED", MediaType, (String), string_parser },
    { "TOAL", OriginalAlbum, (String), string_parser },
    { "TOFN", OriginalFilename, (String), string_parser },
    { "TOLY", OriginalLyricist, (String), string_parser },
    { "TOPE", OriginalArtist, (String), string_parser },
    { "TORY", OriginalReleaseYear, (String), string_parser },
    { "TOWN", Owner, (String), string_parser },
    { "TPE1", LeadArtist, (String), string_parser },
    { "TPE2", Band, (String), string_parser },
    { "TPE3", Conductor, (String), string_parser },
    { "TPE4", Remixed, (String), string_parser },
    { "TPOS", PartOfSet, (String), string_parser },
    { "TPUB", Publisher, (String), string_parser },
    { "TRCK", TrackNumberPosition, (String), string_parser },
    { "TRDA", RecordingDates, (String), string_parser },
    { "TRSN", TRSN, (String), string_parser },
    { "TRSO", TRSO, (String), string_parser },
    { "TSIZ", Size, (String), string_parser },
    { "TSRC", ISRC, (String), string_parser },
    { "TSSE", Software, (String), string_parser },
    { "TYER", Year, (i64), i64_parser },
);

pub(crate) fn parse_payload<R: Read>(
    mut stream: R,
    frame_id: ID3v2FrameID,
    size: usize,
) -> Result<ID3v2Frame, ID3v2ParsePayloadError> {
    let mut buffer = vec![0; size];
    stream.read_exact(&mut buffer)?;

    let frame_id_str = frame_id.as_str().unwrap();

    match ID3V2_FRAME_PARSERS.get(frame_id_str) {
        None => Err(ID3v2ParsePayloadError::UnknownTag(frame_id)),
        Some(parser) => parser(&buffer),
    }
}
