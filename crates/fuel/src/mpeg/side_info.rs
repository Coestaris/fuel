use crate::bite::Biter;
use crate::mpeg::header::{MPEGHeader, MPEGMode, MPEGVersion};
use lazy_static::lazy_static;
use log::debug;
use std::io;
use std::io::Read;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MpegParseSideInfoError {
    #[error("Failed to read from reader: {0}")]
    IOError(#[from] io::Error),
    #[error("Invalid block type: {0}")]
    InvalidBlockType(u32),
}

lazy_static! {}

#[derive(Debug, Clone)]
pub struct MPEGSideInfo {
    pub private_bits: u32,

    pub nch: usize,
    pub ngr: usize,

    pub scfsi: SCFSI,

    pub part2_3_length: PerGranuleData<u32>,
    pub big_values: PerGranuleData<u32>,
    pub global_gain: PerGranuleData<u32>,
    pub scalefac_compress: PerGranuleData<u32>,
    pub window_switching_flag: PerGranuleData<u32>,
    pub block_type: PerGranuleData<u32>,
    pub mixed_block_flag: PerGranuleData<u32>,
    pub table_select: TableSelect,
    pub subblock_gain: SubblockGain,
    pub preflag: PerGranuleData<u32>,
    pub region0_count: PerGranuleData<u32>,
    pub region1_count: PerGranuleData<u32>,
    pub scalefac_scale: PerGranuleData<u32>,
    pub count1table_select: PerGranuleData<u32>,
}

pub const MAX_CHANNELS: usize = 2;
pub const MAX_GRANULES: usize = 2;
pub const MAX_SCFI_BANDS: usize = 4;
pub const MAX_WINDOW: usize = 3;
pub const MAX_TABLE_SELECT: usize = 3;

#[derive(Debug, Clone)]
pub struct SCFSI([u8; MAX_CHANNELS * MAX_SCFI_BANDS]);

impl SCFSI {
    pub fn new() -> Self {
        SCFSI([0; MAX_CHANNELS * MAX_SCFI_BANDS])
    }

    pub fn set(&mut self, channel: usize, band: usize, value: u8) {
        debug_assert!(channel < MAX_CHANNELS);
        debug_assert!(band < MAX_SCFI_BANDS);
        self.0[channel * MAX_SCFI_BANDS + band] = value;
    }

    pub fn get(&self, channel: usize, band: usize) -> u8 {
        debug_assert!(channel < MAX_CHANNELS);
        debug_assert!(band < MAX_SCFI_BANDS);
        self.0[channel * MAX_SCFI_BANDS + band]
    }
}

#[derive(Debug, Clone)]
pub struct PerGranuleData<T>([T; MAX_GRANULES * MAX_CHANNELS]);

impl<T: Default + Sized + Copy> PerGranuleData<T> {
    pub fn new() -> Self {
        PerGranuleData([Default::default(); MAX_GRANULES * MAX_CHANNELS])
    }

    pub fn set(&mut self, granule: usize, channel: usize, value: T) {
        debug_assert!(granule < MAX_GRANULES);
        debug_assert!(channel < MAX_CHANNELS);
        self.0[granule * MAX_CHANNELS + channel] = value;
    }

    pub fn get(&self, granule: usize, channel: usize) -> T {
        debug_assert!(granule < MAX_GRANULES);
        debug_assert!(channel < MAX_CHANNELS);
        self.0[granule * MAX_CHANNELS + channel]
    }
}

#[derive(Debug, Clone)]
pub struct TableSelect([u32; MAX_GRANULES * MAX_CHANNELS * MAX_TABLE_SELECT]);

impl TableSelect {
    pub fn new() -> Self {
        TableSelect([0; MAX_GRANULES * MAX_CHANNELS * MAX_TABLE_SELECT])
    }

    pub fn set(&mut self, granule: usize, channel: usize, region: usize, value: u32) {
        debug_assert!(granule < MAX_GRANULES);
        debug_assert!(channel < MAX_CHANNELS);
        debug_assert!(region < MAX_TABLE_SELECT);

        self.0[granule * MAX_CHANNELS * MAX_TABLE_SELECT + channel * MAX_TABLE_SELECT + region] =
            value;
    }

    pub fn get(&self, granule: usize, channel: usize, region: usize) -> u32 {
        debug_assert!(granule < MAX_GRANULES);
        debug_assert!(channel < MAX_CHANNELS);
        debug_assert!(region < MAX_TABLE_SELECT);

        self.0[granule * MAX_CHANNELS * MAX_TABLE_SELECT + channel * MAX_TABLE_SELECT + region]
    }
}

#[derive(Debug, Clone)]
pub struct SubblockGain([u32; MAX_GRANULES * MAX_CHANNELS * MAX_WINDOW]);

impl SubblockGain {
    pub fn new() -> Self {
        SubblockGain([0; MAX_GRANULES * MAX_CHANNELS * MAX_WINDOW])
    }

    pub fn set(&mut self, granule: usize, channel: usize, window: usize, value: u32) {
        debug_assert!(granule < MAX_GRANULES);
        debug_assert!(channel < MAX_CHANNELS);
        debug_assert!(window < MAX_WINDOW);
        self.0[granule * 6 + channel * 3 + window] = value;
    }

    pub fn get(&self, granule: usize, channel: usize, window: usize) -> u32 {
        debug_assert!(granule < MAX_GRANULES);
        debug_assert!(channel < MAX_CHANNELS);
        debug_assert!(window < MAX_WINDOW);
        self.0[granule * 6 + channel * 3 + window]
    }
}

pub(crate) fn parse_side_info<R: Read, const BUF_SIZE: usize>(
    header: &MPEGHeader,
    biter: &mut Biter<R, BUF_SIZE>,
) -> Result<(MPEGSideInfo, usize), MpegParseSideInfoError> {
    debug!("Parsing MPEG audio data");

    let main_data_begin: u32 = match header.version {
        MPEGVersion::MPEG1 => biter.uimsbf(9)?,
        MPEGVersion::MPEG2 | MPEGVersion::MPEG25 => biter.uimsbf(8)?,
    };
    let private_bits: u32 = match (header.version, header.mode) {
        (MPEGVersion::MPEG1, MPEGMode::SingleChannel) => biter.bslbf(5)?,
        (MPEGVersion::MPEG1, _) => biter.bslbf(3)?,

        (MPEGVersion::MPEG2 | MPEGVersion::MPEG25, MPEGMode::SingleChannel) => biter.bslbf(1)?,
        (MPEGVersion::MPEG2 | MPEGVersion::MPEG25, _) => biter.bslbf(2)?,
    };

    // Number of channels. 1 for single_channel mode, 2 in other modes.
    let nch = match header.mode {
        MPEGMode::SingleChannel => 1,
        _ => 2,
    };

    let mut scfsi = SCFSI::new();
    if matches!(header.version, MPEGVersion::MPEG1) {
        for ch in 0..nch {
            for scfsi_band in 0..MAX_SCFI_BANDS {
                scfsi.set(ch, scfsi_band, biter.bslbf(1)?);
            }
        }
    }

    // Number of granules; equals 2 for MPEG1, 1 for MPEG2 and MPEG2.5.
    let ngr = match header.version {
        MPEGVersion::MPEG1 => 2,
        MPEGVersion::MPEG2 | MPEGVersion::MPEG25 => 1,
    };

    let mut part2_3_length = PerGranuleData::<u32>::new();
    let mut big_values = PerGranuleData::<u32>::new();
    let mut global_gain = PerGranuleData::<u32>::new();
    let mut scalefac_compress = PerGranuleData::<u32>::new();
    let mut window_switching_flag = PerGranuleData::<u32>::new();
    let mut block_type = PerGranuleData::<u32>::new();
    let mut mixed_block_flag = PerGranuleData::<u32>::new();
    let mut table_select = TableSelect::new();
    let mut subblock_gain = SubblockGain::new();
    let mut preflag = PerGranuleData::<u32>::new();
    let mut region0_count = PerGranuleData::<u32>::new();
    let mut region1_count = PerGranuleData::<u32>::new();
    let mut scalefac_scale = PerGranuleData::<u32>::new();
    let mut count1table_select = PerGranuleData::<u32>::new();

    for gr in 0..ngr {
        for ch in 0..nch {
            part2_3_length.set(gr, ch, biter.uimsbf(12)?);
            big_values.set(gr, ch, biter.uimsbf(9)?);
            global_gain.set(gr, ch, biter.uimsbf(8)?);

            if matches!(header.version, MPEGVersion::MPEG1) {
                scalefac_compress.set(gr, ch, biter.uimsbf(4)?);
            } else {
                scalefac_compress.set(gr, ch, biter.uimsbf(9)?);
            }

            let wsf: u32 = biter.bslbf(1)?;
            window_switching_flag.set(gr, ch, wsf);
            if wsf == 1 {
                let bt = biter.uimsbf(2)?;
                let mbf = biter.uimsbf(1)?;

                block_type.set(gr, ch, bt);
                mixed_block_flag.set(gr, ch, mbf);

                if bt == 0 {
                    return Err(MpegParseSideInfoError::InvalidBlockType(bt));
                }

                for region in 0..2 {
                    table_select.set(gr, ch, region, biter.uimsbf(5)?);
                }

                for window in 0..3 {
                    subblock_gain.set(gr, ch, window, biter.uimsbf(3)?);
                }

                if bt == 2 {
                    let r0 = if mbf == 1 { 7 } else { 8 };
                    region0_count.set(gr, ch, r0);
                    region1_count.set(gr, ch, 20 - r0);
                } else {
                    region0_count.set(gr, ch, 7);
                    region1_count.set(gr, ch, 13);
                }
            } else {
                block_type.set(gr, ch, 0);
                mixed_block_flag.set(gr, ch, 0);

                for region in 0..3 {
                    table_select.set(gr, ch, region, biter.uimsbf(5)?);
                }

                region0_count.set(gr, ch, biter.uimsbf(4)?);
                region1_count.set(gr, ch, biter.uimsbf(3)?);
            }

            if matches!(header.version, MPEGVersion::MPEG1) {
                preflag.set(gr, ch, biter.uimsbf(1)?);
            }

            scalefac_scale.set(gr, ch, biter.uimsbf(1)?);
            count1table_select.set(gr, ch, biter.uimsbf(1)?);
        }
    }

    debug_assert!(biter.is_aligned::<32>());

    Ok((
        MPEGSideInfo {
            private_bits,
            nch,
            ngr,
            scfsi,
            part2_3_length,
            big_values,
            global_gain,
            scalefac_compress,
            window_switching_flag,
            block_type,
            mixed_block_flag,
            table_select,
            subblock_gain,
            preflag,
            region0_count,
            region1_count,
            scalefac_scale,
            count1table_select,
        },
        main_data_begin as usize,
    ))
}
