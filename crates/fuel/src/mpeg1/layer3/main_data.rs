use crate::bite::{Biter, BiterError};
use crate::header::MPEGHeader;
use crate::mpeg1::layer3::side_info::{BlockType, GranuleSideInfo, SCFSI, SideInfo};
use log::debug;
use std::io::Read;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DecodeMainDataError {
    #[error("Failed to read from input stream: {0}")]
    BiterError(#[from] BiterError),
    #[error("InvalidBandState")]
    InvalidBandState,
    #[error("Part23Overread")]
    Part23Overread,
}

#[derive(Default, Debug, Clone, Copy)]
pub struct GranuleScaleFactors {
    long: [u8; 21],
    short: [[u8; 12]; 3],
}

#[derive(Debug, Clone, Copy)]
pub struct GranuleSpectralValues([i16; 576]);

impl Default for GranuleSpectralValues {
    fn default() -> Self {
        GranuleSpectralValues([0; 576])
    }
}

#[derive(Debug)]
pub struct MainData {
    pub scale_factors: [[GranuleScaleFactors; 2]; 2],
    pub spectral_values: [[GranuleSpectralValues; 2]; 2],
}

// The scale factors are then further divided
// into two groups, 0-10, 11-20 for long windows and 0-6, 7-11 for short windows.

fn short_group_bits(slen1: u8, slen2: u8, sfb: usize) -> usize {
    if sfb <= 6 {
        slen1 as usize
    } else {
        slen2 as usize
    }
}

fn long_group_bits(slen1: u8, slen2: u8, sfb: usize) -> usize {
    if sfb <= 10 {
        slen1 as usize
    } else {
        slen2 as usize
    }
}

fn decode_scale_factors<R: Read, const BUF_SIZE: usize>(
    prev_granule: Option<GranuleScaleFactors>,
    scfsi: &SCFSI,
    grsi: &GranuleSideInfo,
    biter: &mut Biter<R, BUF_SIZE>,
) -> Result<GranuleScaleFactors, DecodeMainDataError> {
    let mut granule = GranuleScaleFactors::default();

    match (
        grsi.window_switching_flag,
        grsi.block_type,
        grsi.mixed_block_flag,
    ) {
        // Short block with mixed lower long bands.
        (true, BlockType::Short, true) => {
            for sfb in 0..8 {
                let bits = long_group_bits(grsi.slen1, grsi.slen2, sfb);
                granule.long[sfb] = biter.uimsbf(bits)?;
            }

            for sfb in 3..12 {
                let bits = short_group_bits(grsi.slen1, grsi.slen2, sfb);

                for win in 0..3 {
                    granule.short[win][sfb] = biter.uimsbf(bits)?;
                }
            }
        }

        // Pure short block.
        (true, BlockType::Short, false) => {
            for sfb in 0..12 {
                let bits = short_group_bits(grsi.slen1, grsi.slen2, sfb);

                for win in 0..3 {
                    granule.short[win][sfb] = biter.uimsbf(bits)?;
                }
            }
        }

        // Long block, start block, end block.
        //
        // For MPEG1, SCFSI may reuse scalefactors from granule 0
        // when parsing granule 1.
        _ => {
            for sfb in 0..21 {
                if let Some(prev) = prev_granule {
                    if scfsi.is_shared_long_band(sfb) {
                        granule.long[sfb] = prev.long[sfb];
                        continue;
                    }
                }

                let bits = long_group_bits(grsi.slen1, grsi.slen2, sfb);
                granule.long[sfb] = biter.uimsbf(bits)?;
            }
        }
    }

    Ok(granule)
}

fn decode_huffman_bits<R: Read, const BUF_SIZE: usize>(
    _grsi: &GranuleSideInfo,
    bits: usize,
    biter: &mut Biter<R, BUF_SIZE>,
) -> Result<GranuleSpectralValues, DecodeMainDataError> {
    biter.skip_bits(bits)?;
    Ok(GranuleSpectralValues::default())
}

pub(crate) fn decode_main_data<R: Read, const BUF_SIZE: usize>(
    header: &MPEGHeader,
    side_info: &SideInfo,
    biter: &mut Biter<R, BUF_SIZE>,
    main_data_size_bits: usize,
) -> Result<MainData, DecodeMainDataError> {
    let bits_start = biter.bits_read();

    let mut scale_factors = [[GranuleScaleFactors::default(); 2]; 2];
    let mut spectral_values = [[GranuleSpectralValues::default(); 2]; 2];

    for gr in 0..side_info.ngr {
        for ch in 0..side_info.nch {
            let part_start = biter.bits_read();

            let prev = if gr == 1 {
                Some(scale_factors[0][ch])
            } else {
                None
            };

            scale_factors[gr][ch] = decode_scale_factors(
                prev,
                &side_info.scfsi[ch],
                &side_info.granules[gr][ch],
                biter,
            )?;

            let after_scalefac = biter.bits_read();
            let scalefac_bits = after_scalefac - part_start;
            let part2_3_length = side_info.granules[gr][ch].part2_3_length as u64;
            debug!(
                "after scalefac = {}, scalefac_bits = {}",
                after_scalefac, scalefac_bits
            );

            if scalefac_bits > part2_3_length {
                return Err(DecodeMainDataError::Part23Overread);
            }

            let huffman_bits = part2_3_length - scalefac_bits;
            spectral_values[gr][ch] =
                decode_huffman_bits(&side_info.granules[gr][ch], huffman_bits as usize, biter)?;
            debug!("huffman_bits={}, after huffman_bits = {}", huffman_bits, biter.bits_read());
        }
    }

    Ok(MainData {
        scale_factors,
        spectral_values,
    })
}
