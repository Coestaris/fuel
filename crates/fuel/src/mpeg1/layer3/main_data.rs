use crate::bite::Biter;
use crate::header::{MPEGHeader, MPEGVersion};
use crate::mpeg1::layer3::side_info::{
    MAX_CHANNELS, MAX_GRANULES, MAX_WINDOW, MPEGSideInfo, PerGranuleData,
};
use std::io::Read;
use thiserror::Error;

const MAX_SFB: usize = 21;

#[derive(Debug, Clone)]
struct ScaleFactorL([u32; MAX_GRANULES * MAX_CHANNELS * MAX_SFB]);

impl ScaleFactorL {
    pub fn new() -> Self {
        ScaleFactorL([0; MAX_GRANULES * MAX_CHANNELS * MAX_SFB])
    }

    pub fn set(&mut self, granule: usize, channel: usize, sfb: usize, value: u32) {
        debug_assert!(granule < MAX_GRANULES);
        debug_assert!(channel < MAX_CHANNELS);
        debug_assert!(sfb < MAX_SFB);
        self.0[granule * MAX_CHANNELS * MAX_SFB + channel * MAX_SFB + sfb] = value;
    }

    pub fn get(&self, granule: usize, channel: usize, sfb: usize) -> u32 {
        debug_assert!(granule < MAX_GRANULES);
        debug_assert!(channel < MAX_CHANNELS);
        debug_assert!(sfb < MAX_SFB);
        self.0[granule * MAX_CHANNELS * MAX_SFB + channel * MAX_SFB + sfb]
    }
}

#[derive(Debug, Clone)]
struct ScaleFactorS([u32; MAX_GRANULES * MAX_CHANNELS * MAX_SFB * MAX_WINDOW]);

impl ScaleFactorS {
    pub fn new() -> Self {
        ScaleFactorS([0; MAX_GRANULES * MAX_CHANNELS * MAX_SFB * MAX_WINDOW])
    }

    pub fn set(&mut self, granule: usize, channel: usize, sfb: usize, window: usize, value: u32) {
        debug_assert!(granule < MAX_GRANULES);
        debug_assert!(channel < MAX_CHANNELS);
        debug_assert!(sfb < MAX_SFB);
        debug_assert!(window < MAX_WINDOW);
        self.0[granule * MAX_CHANNELS * MAX_SFB * MAX_WINDOW
            + channel * MAX_SFB * MAX_WINDOW
            + sfb * MAX_WINDOW
            + window] = value;
    }

    pub fn get(&self, granule: usize, channel: usize, sfb: usize, window: usize) -> u32 {
        debug_assert!(granule < MAX_GRANULES);
        debug_assert!(channel < MAX_CHANNELS);
        debug_assert!(sfb < MAX_SFB);
        debug_assert!(window < MAX_WINDOW);
        self.0[granule * MAX_CHANNELS * MAX_SFB * MAX_WINDOW
            + channel * MAX_SFB * MAX_WINDOW
            + sfb * MAX_WINDOW
            + window]
    }
}

#[derive(Debug, Error)]
pub enum MpegParseMainDataError {
    #[error("Failed to read main data: {0}")]
    Read(#[from] std::io::Error),
}

#[derive(Debug, Clone)]
pub struct MPEGMainData {
    pub scale_factor_l: ScaleFactorL,
    pub scale_factor_s: ScaleFactorS,
    pub huffman_code_bits: PerGranuleData<HuffmanCodeBits>,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct HuffmanCodeBits {}

fn parse_huffman_code_bits<R: Read, const BUF_SIZE: usize>(
    biter: &mut Biter<R, BUF_SIZE>,
) -> Result<HuffmanCodeBits, MpegParseMainDataError> {
    Ok(HuffmanCodeBits {})
}

pub(crate) fn parse_main_data<R: Read, const BUF_SIZE: usize>(
    header: &MPEGHeader,
    side_info: &MPEGSideInfo,
    main_data_begin: usize,
    biter: &mut Biter<R, BUF_SIZE>,
) -> Result<MPEGMainData, MpegParseMainDataError> {
    let mut scale_factor_l = ScaleFactorL::new();
    let mut scale_factor_s = ScaleFactorS::new();
    let mut huffman_code_bits = PerGranuleData::<HuffmanCodeBits>::new();

    for gr in 0..side_info.ngr {
        for ch in 0..side_info.nch {
            if (side_info.window_switching_flag.get(gr, ch) == 1)
                && (side_info.block_type.get(gr, ch) == 0b10)
            {
                if side_info.mixed_block_flag.get(gr, ch) == 0b1 {
                    for sfb in 0..8 {
                        scale_factor_l.set(gr, ch, sfb, biter.uimsbf(4)?);
                    }

                    for sfb in 3..12 {
                        for window in 0..MAX_WINDOW {
                            scale_factor_s.set(gr, ch, sfb, window, biter.uimsbf(4)?);
                        }
                    }
                } else {
                    for sfb in 0..12 {
                        for window in 0..MAX_WINDOW {
                            scale_factor_s.set(gr, ch, sfb, window, biter.uimsbf(4)?);
                        }
                    }
                }
            } else {
                if side_info.scfsi.get(ch, 0) == 0 || gr == 0 {
                    for sfb in 0..6 {
                        scale_factor_l.set(gr, ch, sfb, biter.uimsbf(4)?);
                    }
                }
                if side_info.scfsi.get(ch, 1) == 0 || gr == 0 {
                    for sfb in 6..11 {
                        scale_factor_l.set(gr, ch, sfb, biter.uimsbf(4)?);
                    }
                }
                if side_info.scfsi.get(ch, 2) == 0 || gr == 0 {
                    for sfb in 11..16 {
                        scale_factor_l.set(gr, ch, sfb, biter.uimsbf(3)?);
                    }
                }
                if side_info.scfsi.get(ch, 3) == 0 || gr == 0 {
                    for sfb in 16..21 {
                        scale_factor_l.set(gr, ch, sfb, biter.uimsbf(3)?);
                    }
                }
            }

            huffman_code_bits.set(gr, ch, parse_huffman_code_bits(biter)?)
        }
    }

    Ok(MPEGMainData {
        scale_factor_l,
        scale_factor_s,
        huffman_code_bits,
    })
}
