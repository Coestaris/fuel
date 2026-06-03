use crate::bite::Biter;
use crate::header::{MPEGHeader, MPEGMode};
use lazy_static::lazy_static;
use log::debug;
use std::collections::HashMap;
use std::io;
use std::io::Read;
use thiserror::Error;

#[derive(Debug, Clone, Default)]
pub enum BlockType {
    #[default]
    Forbidden,
    Start,
    ShortWindows {
        subblock_gain: [u32; 3],
    },
    End,
}

#[derive(Debug, Clone, Default)]
pub struct Granule {
    block_type: BlockType,

    // Determines the number of bits used for the transmission of scalefactors. A granule can be
    // divided into 12 or 21 scalefactor bands. If long windows are used (block_type = {0,1,3})the
    // granule will be partitioned into 21 scalefactor bands. Using short windows (block_type = 2)
    // will partition the granule into 12 scalefactor bands. The scale factors are then further divided
    // into two groups, 0-10, 11-20 for long windows and 0-6, 7-11 for short windows.
    slen1: u32,
    slen2: u32,

    // States the number of bits allocated in the main data part of the frame for scalefactors (part2)
    // and Huffman encoded data (part3). 12 bits will be used in a single channel mode whereas in
    // stereo modes the double is needed. This field can be used to calculate the location of the next
    // granule and the ancillary information (if used).
    part2_3_length: u32,

    // Specifies the quantization step size, this is needed in the requantization block of the decoder.
    global_gain: u32,
    scalefac_scale: u32,

    // region 0 | region 1 | region 2 || count 1            || rzero                  ||
    // 1                              || big values * 2     || big_values*2+count1*4  ||576
    // The big_values field indicates the size of the big_values partition hence
    // the maximum value is 288.
    big_values: u32,
    region0_count: u32,
    region1_count: u32,

    region0_table_index: u32,
    region1_table_index: u32,
    region2_table_index: u32,
    count1_table_index: u32,

    // This is a shortcut for additional high frequency amplification of the
    // quantized values. If preflag[gr][ch] is set, the values of a table are added to the
    // scalefactors (see Table 23). This is equivalent to multiplication of the requantized
    // scalefactors with table values. If (block_type[gr][ch]==’10’) preflag[gr][ch] is never
    // used.
    preflag: bool,
    mixed_block_flag: bool,
}

#[derive(Debug, Clone, Default)]
pub struct SCFSI(u8);

impl SCFSI {
    fn new(value: u8) -> Self {
        Self(value)
    }

    fn new_from_bits(a: bool, b: bool, c: bool, d: bool) -> Self {
        Self::new(a as u8 | (b as u8) << 1 | (c as u8) << 2 | (d as u8) << 3)
    }

    fn is_shared(&self, index: usize) -> bool {
        ((self.0 & 0b0001 == 1) && index >= 0 && index <= 5)
            && ((self.0 & 0b0010 == 1) && index >= 6 && index <= 10)
            && ((self.0 & 0b0100 == 1) && index >= 11 && index <= 15)
            && ((self.0 & 0b1000 == 1) && index >= 16 && index <= 20)
    }
}

#[derive(Debug, Clone)]
pub struct MPEGSideInfo {
    pub main_data_begin: u32,

    // Bits for private use. These bits will not be used in the future by ISO/IEC.
    pub private_bits: u32,

    // Number of channels
    pub nch: usize,
    // Number of granules (always 2)
    pub ngr: usize,

    // Scalefactor selection information
    // Controls the application of scalefactors to granules. The scalefactor select information
    // indicates whether scalefactors are transferred for granule1, or not.
    // If short windows are used (block_type = 10) in any granule/channel, the scalefactors are
    // always sent for each granule for that channel.
    pub scfsi: [SCFSI; 2],

    pub granule0: [Granule; 2],
    pub granule1: [Granule; 2],
}

#[derive(Debug, Error)]
pub enum MpegParseSideInfoError {
    #[error("Failed to read from reader: {0}")]
    IOError(#[from] io::Error),
    #[error("Invalid block type: {0}")]
    InvalidBlockType(u32),
}

lazy_static! {
    static ref SCALEFAC_COMPRESS_TO_SLEN: HashMap<u32, (u32, u32)> = {
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
    Ok(SCFSI::new(biter.uimsbf(4)?))
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
            0b10 => BlockType::ShortWindows {
                subblock_gain: [0; 3],
            },
            0b11 => BlockType::End,
            bt => Err(MpegParseSideInfoError::InvalidBlockType(bt))?,
        };

        granule.mixed_block_flag = biter.uimsbf(1)?;
        granule.region0_table_index = biter.uimsbf(5)?;
        granule.region1_table_index = biter.uimsbf(5)?;
        granule.region2_table_index = biter.uimsbf(5)?;

        for window in 0..3 {
            let sbg = biter.uimsbf(3)?;
            if let BlockType::ShortWindows { subblock_gain } = &mut granule.block_type {
                subblock_gain[window] = sbg;
            }
        }

        if matches!(
            granule.block_type,
            BlockType::ShortWindows { subblock_gain: _ }
        ) {
            let r0 = if granule.mixed_block_flag { 7 } else { 8 };
            granule.region0_count = r0;
            granule.region1_count = 20 - r0;
        } else {
            granule.region0_count = 7;
            granule.region1_count = 13;
        }
    } else {
        granule.region0_table_index = biter.uimsbf(5)?;
        granule.region1_table_index = biter.uimsbf(5)?;
        granule.region2_table_index = biter.uimsbf(5)?;

        granule.region0_count = biter.uimsbf(4)?;
        granule.region1_count = biter.uimsbf(3)?;
    }

    granule.preflag = biter.uimsbf(1)?;
    granule.scalefac_scale = biter.uimsbf(1)?;
    granule.count1_table_index = biter.uimsbf(1)?;

    Ok(granule)
}

pub(crate) fn parse_side_info<R: Read, const BUF_SIZE: usize>(
    header: &MPEGHeader,
    biter: &mut Biter<R, BUF_SIZE>,
) -> Result<MPEGSideInfo, MpegParseSideInfoError> {
    debug!("Parsing MPEG audio data");

    // Number of channels. 1 for single_channel mode, 2 in other modes.
    let nch = match header.mode {
        MPEGMode::SingleChannel => 1,
        _ => 2,
    };

    let main_data_begin: u32 = biter.uimsbf(9)?;
    let private_bits: u32 = if nch == 1 {
        biter.bslbf(5)?
    } else {
        biter.bslbf(3)?
    };

    Ok(MPEGSideInfo {
        main_data_begin,
        private_bits,
        nch,
        ngr: 2,
        scfsi: [
            parse_scfsi(biter)?,
            if nch == 2 {
                parse_scfsi(biter)?
            } else {
                SCFSI::default()
            },
        ],
        granule0: [
            parse_granule(biter)?,
            if nch == 2 {
                parse_granule(biter)?
            } else {
                Default::default()
            },
        ],
        granule1: [
            parse_granule(biter)?,
            if nch == 2 {
                parse_granule(biter)?
            } else {
                Default::default()
            },
        ],
    })
}
