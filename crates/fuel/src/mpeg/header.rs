use crate::bite::Biter;
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::fmt::Debug;
use std::io;
use std::io::{Read, Seek};
use log::debug;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MpegParseHeaderError {
    #[error("Failed to read from reader: {0}")]
    IOError(#[from] io::Error),
    #[error("Failed to parse header: {0}")]
    InvalidSyncWord(u32),
    #[error("Unsupported MPEG version combination. ID={0}, IDex={1}")]
    UnsupportedMPEGVersion(u32, u32),
    #[error("Unsupported layer: {0}")]
    UnsupportedLayer(u32),
    #[error("Unsupported Bitrate combination. ID={0}, IDex={1}, bitrate={2}")]
    BitrateError(u32, u32, u32),
    #[error("Unsupported sampling frequency combination. ID={0}, IDex={1}, sampling frequency={2}")]
    UnsupportedSamplingFrequency(u32, u32, u32),
    #[error("Unsupported emphasis type: {0}")]
    UnsupportedEmphasisType(u32),
    #[error("Unsupported CRC: {0}")]
    UnsupportedCRC(u32),
}

#[derive(Copy, Clone, Debug)]
pub enum MPEGVersion {
    MPEG1,
    MPEG2,
    MPEG25,
}

#[derive(Copy, Clone, Debug)]
pub enum MPEGLayer {
    LayerI,
    LayerII,
    LayerIII,
}

#[derive(Copy, Clone, Debug)]
pub enum MPEGBitrate {
    Free,
    Fixed(u32),
}

#[derive(Copy, Clone, Debug)]

pub enum MPEGMode {
    Stereo,
    JointStereo {
        intensity_stereo: bool,
        ms_stereo: bool,
    },
    DualChannel,
    SingleChannel,
}

#[derive(Copy, Clone, Debug)]
pub enum MPEGEmphasis {
    None,
    _50_15ms,
    CCIT_J17,
}

const SYNCWORD: u32 = 0x7FF;
const INVALID_CRC: u32 = 0x0000;

lazy_static! {
    static ref MPEG_VERSIONS: HashMap<(u32, u32), MPEGVersion> = {
        let mut m = HashMap::new();
        m.insert((0, 0), MPEGVersion::MPEG25);
        m.insert((1, 0), MPEGVersion::MPEG2);
        m.insert((1, 1), MPEGVersion::MPEG1);
        m
    };

    static ref MPEG_LAYERS: HashMap<u32, MPEGLayer> = {
        let mut m = HashMap::new();
        m.insert(0b1, MPEGLayer::LayerIII);
        m
    };

    static ref BITRATE_TABLE: HashMap<(u32, u32, u32), MPEGBitrate> = {
        let mut m = HashMap::new();

        m.insert((1, 1, 0b0000), MPEGBitrate::Free);
        m.insert((1, 1, 0b0001), MPEGBitrate::Fixed(32));
        m.insert((1, 1, 0b0010), MPEGBitrate::Fixed(40));
        m.insert((1, 1, 0b0011), MPEGBitrate::Fixed(48));
        m.insert((1, 1, 0b0100), MPEGBitrate::Fixed(56));
        m.insert((1, 1, 0b0101), MPEGBitrate::Fixed(64));
        m.insert((1, 1, 0b0110), MPEGBitrate::Fixed(80));
        m.insert((1, 1, 0b0111), MPEGBitrate::Fixed(96));
        m.insert((1, 1, 0b1000), MPEGBitrate::Fixed(112));
        m.insert((1, 1, 0b1001), MPEGBitrate::Fixed(128));
        m.insert((1, 1, 0b1010), MPEGBitrate::Fixed(160));
        m.insert((1, 1, 0b1011), MPEGBitrate::Fixed(192));
        m.insert((1, 1, 0b1100), MPEGBitrate::Fixed(224));
        m.insert((1, 1, 0b1101), MPEGBitrate::Fixed(256));
        m.insert((1, 1, 0b1110), MPEGBitrate::Fixed(320));
        // (1, 1, 0b1111) - forbidden
        m.insert((0, 1, 0b0000), MPEGBitrate::Free);
        m.insert((0, 1, 0b0001), MPEGBitrate::Fixed(8));
        m.insert((0, 1, 0b0010), MPEGBitrate::Fixed(16));
        m.insert((0, 1, 0b0011), MPEGBitrate::Fixed(24));
        m.insert((0, 1, 0b0100), MPEGBitrate::Fixed(32));
        m.insert((0, 1, 0b0101), MPEGBitrate::Fixed(40));
        m.insert((0, 1, 0b0110), MPEGBitrate::Fixed(48));
        m.insert((0, 1, 0b0111), MPEGBitrate::Fixed(56));
        m.insert((0, 1, 0b1000), MPEGBitrate::Fixed(64));
        m.insert((0, 1, 0b1001), MPEGBitrate::Fixed(80));
        m.insert((0, 1, 0b1010), MPEGBitrate::Fixed(96));
        m.insert((0, 1, 0b1011), MPEGBitrate::Fixed(112));
        m.insert((0, 1, 0b1100), MPEGBitrate::Fixed(128));
        m.insert((0, 1, 0b1101), MPEGBitrate::Fixed(144));
        m.insert((0, 1, 0b1110), MPEGBitrate::Fixed(160));
        // (0, 1, 0b1111) - forbidden
        m.insert((0, 0,    0b0000), MPEGBitrate::Free);
        m.insert((0, 0, 0b0001), MPEGBitrate::Fixed(8));
        m.insert((0, 0, 0b0010), MPEGBitrate::Fixed(16));
        m.insert((0, 0, 0b0011), MPEGBitrate::Fixed(24));
        m.insert((0, 0, 0b0100), MPEGBitrate::Fixed(32));
        m.insert((0, 0, 0b0101), MPEGBitrate::Fixed(40));
        m.insert((0, 0, 0b0110), MPEGBitrate::Fixed(48));
        m.insert((0, 0, 0b0111), MPEGBitrate::Fixed(56));
        m.insert((0, 0, 0b1000), MPEGBitrate::Fixed(64));
        // ((0, 0, 0b1001) - forbidden
        // ((0, 0, 0b1010) - forbidden
        // ((0, 0, 0b1011) - forbidden
        // ((0, 0, 0b1100) - forbidden
        // ((0, 0, 0b1101) - forbidden
        // ((0, 0, 0b1110) - forbidden
        // ((0, 0, 0b1111) - forbidden
        m
    };

    static ref SAMPLING_FREQUNCY_TABLE: HashMap<(u32, u32, u32), u32> = {
        let mut m = HashMap::new();

        m.insert((1, 1, 0b00), 44100);
        m.insert((1, 1, 0b01), 48000);
        m.insert((1, 1, 0b10), 32000);
        // ((0, 0, 0b11) - reserved

        m.insert((1, 0, 0b00), 22050);
        m.insert((1, 0, 0b01), 24000);
        m.insert((1, 0, 0b10), 16000);
        // ((1, 0, 0b11) - reserved

        m.insert((0, 0, 0b00), 11025);
        m.insert((0, 0, 0b01), 12000);
        m.insert((0, 0, 0b10), 8000);
        // ((1, 1, 0b11) - reserved

        m
    };

    static ref MODE_TALBE : HashMap<(u32, u32), MPEGMode> = {
        let mut m = HashMap::new();
        m.insert((0b00, 0b00), MPEGMode::Stereo);
        m.insert((0b01, 0b00), MPEGMode::JointStereo { intensity_stereo: false, ms_stereo: false });
        m.insert((0b01, 0b01), MPEGMode::JointStereo { intensity_stereo: true, ms_stereo: false });
        m.insert((0b01, 0b10), MPEGMode::JointStereo { intensity_stereo: false, ms_stereo: true });
        m.insert((0b01, 0b11), MPEGMode::JointStereo { intensity_stereo: false, ms_stereo: true });
        m.insert((0b10, 0b00), MPEGMode::DualChannel);
        m.insert((0b11, 0b00), MPEGMode::SingleChannel);
        m
    };

    static ref EMPHASIS_TABLE: HashMap<u32, MPEGEmphasis> = {
        let mut m = HashMap::new();
        m.insert(0b00, MPEGEmphasis::None);
        m.insert(0b01, MPEGEmphasis::_50_15ms);
        m.insert(0b10, MPEGEmphasis::CCIT_J17);
        // 0b11 - reserved
        m
    };
}

#[derive(Debug, Clone)]
pub struct MPEGHeader {
    pub version: MPEGVersion,
    pub layer: MPEGLayer,
    pub bitrate: MPEGBitrate,
    pub sampling_frequency: u32,
    pub padding_bit: bool,
    pub private_bit: bool,
    pub mode: MPEGMode,
    pub is_copyright: bool,
    pub is_original: bool,
    pub emphasis: MPEGEmphasis,
    pub crc_word: Option<u16>,
}

pub(crate) fn parse_header<R: Read, const BUF_SIZE: usize>(
    biter: &mut Biter<R, BUF_SIZE>,
) -> Result<MPEGHeader, MpegParseHeaderError> {
    debug!("Parsing MPEG frame header");

    /* Get bits from buffer */
    let syncword: u32 = biter.bslbf(11)?;
    let idex: u32 = biter.bslbf(1)?;
    let id: u32 = biter.bslbf(1)?;
    let layer: u32 = biter.bslbf(2)?;
    let protection_bit: bool = biter.bslbf(1)?;
    let bitrate_index: u32 = biter.bslbf(4)?;
    let sampling_frequency: u32 = biter.bslbf(2)?;
    let padding_bit: bool = biter.bslbf(1)?;
    let private_bit: bool = biter.bslbf(1)?;
    let mode: u32 = biter.bslbf(2)?;
    let mode_extension: u32 = biter.bslbf(2)?;
    let copyright: bool = biter.bslbf(1)?;
    let original_copy: bool = biter.bslbf(1)?;
    let emphasis: u32 = biter.bslbf(2)?;

    if syncword != SYNCWORD {
        Err(MpegParseHeaderError::InvalidSyncWord(syncword))?;
    }

    let crc_word: Option<u16> = if !protection_bit {
        Some(biter.uimsbf(16)?)
    } else {
        None
    };

    assert_eq!(biter.is_aligned::<16>(), true);

    let header = MPEGHeader {
        version: *MPEG_VERSIONS
            .get(&(idex, id))
            .ok_or_else(|| MpegParseHeaderError::UnsupportedMPEGVersion(id, idex))?,
        layer: *MPEG_LAYERS
            .get(&layer)
            .ok_or_else(|| MpegParseHeaderError::UnsupportedLayer(layer))?,
        bitrate: *BITRATE_TABLE
            .get(&(idex, id, bitrate_index))
            .ok_or_else(|| MpegParseHeaderError::BitrateError(id, idex, bitrate_index))?,
        sampling_frequency: *SAMPLING_FREQUNCY_TABLE
            .get(&(idex, id, sampling_frequency))
            .ok_or_else(|| {
                MpegParseHeaderError::UnsupportedSamplingFrequency(
                    id,
                    idex,
                    sampling_frequency,
                )
            })?,
        padding_bit,
        private_bit,
        mode: *MODE_TALBE.get(&(mode, mode_extension)).ok_or_else(|| {
            MpegParseHeaderError::UnsupportedMPEGVersion(mode, mode_extension)
        })?,
        is_original: original_copy,
        is_copyright: copyright,
        emphasis: *EMPHASIS_TABLE
            .get(&emphasis)
            .ok_or_else(|| MpegParseHeaderError::UnsupportedEmphasisType(emphasis))?,
        crc_word,
    };

    Ok(header)
}
