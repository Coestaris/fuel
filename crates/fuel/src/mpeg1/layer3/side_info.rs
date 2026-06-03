use crate::bite::Biter;
use crate::header::{MPEGHeader, MPEGMode};
use lazy_static::lazy_static;
use log::debug;
use std::collections::HashMap;
use std::io;
use std::io::Read;
use thiserror::Error;

/// MPEG Layer III block/window type for one granule/channel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BlockType {
    /// Normal long block. Used when `window_switching_flag == false`.
    #[default]
    Long,
    /// Start block: transition from long to short windows.
    Start,
    /// Short block: three short windows per granule.
    Short,
    /// End block: transition from short back to long windows.
    End,
}

/// Scalefactor selection info for MPEG1 Layer III.
/// Used only for granule 1. If a band group is shared,
/// scalefactors are reused from granule 0 instead of being read again.
#[derive(Debug, Clone, Default)]
pub struct SCFSI(u8);

/// Side information for one Layer III granule/channel.
/// This does not contain decoded samples. It only describes how to read
/// scalefactors and Huffman data from `main_data`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Granule {
    /// Number of bits occupied by scalefactors + Huffman data
    /// for this granule/channel in `main_data`.
    pub part2_3_length: u16,

    /// Number of Huffman value pairs in big_values region.
    /// Covers spectral samples `0 .. big_values * 2`.
    pub big_values: u16,

    /// Base quantizer gain used during requantization.
    pub global_gain: u8,

    /// Raw scalefactor compression field from side info.
    /// MPEG1: 4 bits, maps to `(slen1, slen2)`.
    /// MPEG2/2.5: 9 bits, has different meaning.
    pub scalefac_compress: u16,

    /// Number of bits per scalefactor in the first scalefactor group.
    /// Derived from `scalefac_compress` for MPEG1.
    pub slen1: u8,

    /// Number of bits per scalefactor in the second scalefactor group.
    /// Derived from `scalefac_compress` for MPEG1.
    pub slen2: u8,

    /// Whether this granule uses switched window syntax.
    /// If true, `block_type`, `mixed_block_flag`, two table_selects
    /// and three subblock gains are present.
    pub window_switching_flag: bool,

    /// Long/short/start/end window mode for this granule.
    pub block_type: BlockType,

    /// Mixed block: lower bands use long windows, upper bands use short windows.
    /// Meaningful mainly with `block_type == Short`.
    pub mixed_block_flag: bool,

    /// Huffman table indices for regions 0, 1, 2.
    /// Normal block: all 3 are read.
    /// Switched block: only [0] and [1] are read; [2] should stay 0.
    pub table_select: [u8; 3],

    /// Gain correction for each short window.
    /// Present only when `window_switching_flag == true`.
    pub subblock_gain: [u8; 3],

    /// Number of scalefactor bands in region 0 minus one.
    /// Used to split big_values into Huffman regions.
    pub region0_count: u8,

    /// Number of scalefactor bands in region 1 minus one.
    /// Region 2 is whatever remains after region 0 and region 1.
    pub region1_count: u8,

    /// MPEG1-only high-frequency preemphasis flag.
    /// If set, decoder adds predefined pretab values to long-block scalefactors.
    pub preflag: bool,

    /// Scalefactor step size selector.
    ///      false: 0.5 dB steps.
    ///      true:  1.0 dB steps.
    pub scalefac_scale: bool,

    /// Selects Huffman table A/B for count1 region.
    ///      false: table 32.
    ///      true:  table 33.
    pub count1table_select: bool,
}

/// MPEG Layer III side information for one frame.
/// This is read immediately after frame header and optional CRC.
/// For MPEG1 Layer III:
/// - stereo: 32 bytes
/// - mono:   17 bytes
#[derive(Debug, Clone, Default)]
pub struct MPEGSideInfo {
    /// Backpointer into bit reservoir, in bytes.
    /// Tells how many bytes before current frame's main_data the actual
    /// main_data for this frame begins.
    pub main_data_begin: u16,

    /// Encoder-private side info bits.
    /// Not used by the ISO decoder pipeline.
    pub private_bits: u8,

    /// Number of channels:
    /// 1 for SingleChannel, 2 for Stereo/JointStereo/DualChannel.
    pub nch: usize,

    /// Number of granules:
    /// MPEG1 Layer III: 2
    /// MPEG2/2.5 Layer III: 1
    pub ngr: usize,

    /// MPEG1 scalefactor reuse flags, one per channel.
    /// Only meaningful for MPEG1 and granule 1.
    pub scfsi: [SCFSI; 2],

    /// Granule side info indexed as `[granule][channel]`.
    /// Valid range:
    /// `gr < ngr`, `ch < nch`.
    pub granules: [[Granule; 2]; 2],
}
#[derive(Debug, Error)]
pub enum MpegParseSideInfoError {
    #[error("Failed to read from reader: {0}")]
    IOError(#[from] io::Error),
    #[error("Invalid block type: {0}")]
    InvalidBlockType(u32),
}

impl SCFSI {
    fn new(value: u8) -> Self {
        Self(value)
    }

    fn new_from_bits(a: bool, b: bool, c: bool, d: bool) -> Self {
        Self::new(a as u8 | (b as u8) << 1 | (c as u8) << 2 | (d as u8) << 3)
    }

    fn is_shared_long_band(&self, sfb: usize) -> bool {
        match sfb {
            0..=5 => (self.0 & 0b0001) != 0,
            6..=10 => (self.0 & 0b0010) != 0,
            11..=15 => (self.0 & 0b0100) != 0,
            16..=20 => (self.0 & 0b1000) != 0,
            _ => false,
        }
    }
}

lazy_static! {
    static ref SCALEFAC_COMPRESS_TO_SLEN: HashMap<u32, (u8, u8)> = {
        let mut map = HashMap::new();

        map.insert(0, (0, 0));
        map.insert(1, (0, 1));
        map.insert(2, (0, 2));
        map.insert(3, (0, 3));
        map.insert(4, (3, 0));
        map.insert(5, (1, 1));
        map.insert(6, (1, 2));
        map.insert(7, (1, 3));
        map.insert(8, (2, 1));
        map.insert(9, (2, 2));
        map.insert(10, (2, 3));
        map.insert(11, (3, 1));
        map.insert(12, (3, 2));
        map.insert(13, (3, 3));
        map.insert(14, (4, 2));
        map.insert(15, (4, 3));

        map
    };
}

fn parse_scfsi<R: Read, const BUF_SIZE: usize>(
    biter: &mut Biter<R, BUF_SIZE>,
) -> Result<SCFSI, MpegParseSideInfoError> {
    Ok(SCFSI::new_from_bits(
        biter.uimsbf(1)?,
        biter.uimsbf(1)?,
        biter.uimsbf(1)?,
        biter.uimsbf(1)?,
    ))
}

fn parse_granule<R: Read, const BUF_SIZE: usize>(
    biter: &mut Biter<R, BUF_SIZE>,
) -> Result<Granule, MpegParseSideInfoError> {
    let mut granule = Granule::default();

    granule.part2_3_length = biter.uimsbf(12)?;
    granule.big_values = biter.uimsbf(9)?;
    granule.global_gain = biter.uimsbf(8)?;

    let scalefac_compress = biter.uimsbf(4)?;
    let slen = SCALEFAC_COMPRESS_TO_SLEN.get(&scalefac_compress).unwrap();
    granule.slen1 = slen.0;
    granule.slen2 = slen.1;

    let wsf: u32 = biter.bslbf(1)?;
    if wsf == 1 {
        granule.block_type = match biter.uimsbf(2)? {
            0b01 => BlockType::Start,
            0b10 => BlockType::Short,
            0b11 => BlockType::End,
            bt => return Err(MpegParseSideInfoError::InvalidBlockType(bt)),
        };

        granule.mixed_block_flag = biter.uimsbf(1)?;

        granule.table_select[0] = biter.uimsbf(5)?;
        granule.table_select[1] = biter.uimsbf(5)?;
        granule.table_select[2] = 0;

        for window in 0..3 {
            let sbg = biter.uimsbf(3)?;
            granule.subblock_gain[window] = sbg;
        }

        if matches!(granule.block_type, BlockType::Short { .. }) {
            let r0 = if granule.mixed_block_flag { 7 } else { 8 };
            granule.region0_count = r0;
            granule.region1_count = 20 - r0;
        } else {
            granule.region0_count = 7;
            granule.region1_count = 13;
        }
    } else {
        granule.block_type = BlockType::Long;

        granule.table_select[0] = biter.uimsbf(5)?;
        granule.table_select[1] = biter.uimsbf(5)?;
        granule.table_select[2] = biter.uimsbf(5)?;

        granule.region0_count = biter.uimsbf(4)?;
        granule.region1_count = biter.uimsbf(3)?;
    }

    granule.preflag = biter.uimsbf(1)?;
    granule.scalefac_scale = biter.uimsbf(1)?;
    granule.count1table_select = biter.uimsbf(1)?;

    Ok(granule)
}

pub(crate) fn parse_side_info<R: Read, const BUF_SIZE: usize>(
    header: &MPEGHeader,
    biter: &mut Biter<R, BUF_SIZE>,
) -> Result<MPEGSideInfo, MpegParseSideInfoError> {
    debug!("Parsing MPEG audio data");

    let nch = match header.mode {
        MPEGMode::SingleChannel => 1,
        _ => 2,
    };

    let main_data_begin: u16 = biter.uimsbf(9)?;
    let private_bits: u8 = if nch == 1 {
        biter.bslbf(5)?
    } else {
        biter.bslbf(3)?
    };

    let scfsi = [
        parse_scfsi(biter)?,
        if nch == 2 {
            parse_scfsi(biter)?
        } else {
            SCFSI::default()
        },
    ];

    let mut granules = [
        [Granule::default(), Granule::default()],
        [Granule::default(), Granule::default()],
    ];

    for gr in 0..2 {
        for ch in 0..nch {
            granules[gr][ch] = parse_granule(biter)?;
        }
    }

    Ok(MPEGSideInfo {
        main_data_begin,
        private_bits,
        nch,
        ngr: 2,
        scfsi,
        granules,
    })
}
