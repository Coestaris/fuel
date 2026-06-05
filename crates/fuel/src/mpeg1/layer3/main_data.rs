use crate::bite::{Biter, BiterError};
use crate::header::MPEGHeader;
use crate::mpeg1::layer3::huffman::{BigHuffmanDecoder, BigHuffmanDecoderResult, BigHuffmanTable};
use crate::mpeg1::layer3::side_info::{BlockType, GranuleSideInfo, SCFSI, SideInfo};
use lazy_static::lazy_static;
use log::debug;
use std::collections::HashMap;
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
    #[error("InvalidBigHuffmanCode")]
    InvalidBigHuffmanCode,
    #[error("InvalidCount1HuffmanCode")]
    InvalidCount1HuffmanCode,
}

struct SpectralMap<'a> {
    // (sfb, region index)
    map: [(u8, u8); 576],
    grsi: &'a GranuleSideInfo,
}

impl SpectralMap<'_> {
    const FREQ_8KHZ: usize = 0;
    const FREQ_11_025KHZ: usize = 1;
    const FREQ_12KHZ: usize = 2;
    const FREQ_16KHZ: usize = 3;
    const FREQ_22_05KHZ: usize = 4;
    const FREQ_24KHZ: usize = 5;
    const FREQ_32KHZ: usize = 6;
    const FREQ_44_1KHZ: usize = 7;
    const FREQ_48KHZ: usize = 8;
    const BLOCK_LONG_INDEX: usize = 0;
    const BLOCK_SHORT_INDEX: usize = 1;
    const REGION0: u8 = 0;
    const REGION1: u8 = 1;
    const REGION2_OR_OTHERS: u8 = 2;

    // SCALE_FACTOR_BANDS[KHZ_INDEX][BLOCK_TYPE_INDEX][SFB] - width of band
    #[rustfmt::skip]
    const SCALE_FACTOR_BANDS: [[[u8; 21]; 2]; 9] = [
        [ [ 12, 12, 12, 12, 12, 12, 16, 20, 24, 28, 32, 40, 48, 56, 64, 76, 90, 2, 2, 2, 2 ],
          [ 8, 8, 8, 12, 16, 20, 24, 28, 36, 2, 2, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0] ],
        [ [ 6, 6, 6, 6, 6, 6, 8, 10, 12, 14, 16, 20, 24, 28, 32, 38, 46, 52, 60, 68, 58 ],
          [ 4, 4, 4, 6, 8, 10, 12, 14, 18, 24, 30, 40, 0, 0, 0, 0, 0, 0, 0, 0, 0 ] ],
        [ [ 6, 6, 6, 6, 6, 6, 8, 10, 12, 14, 16, 20, 24, 28, 32, 38, 46, 52, 60, 68, 58 ],
          [ 4, 4, 4, 6, 8, 10, 12, 14, 18, 24, 30, 40, 0, 0, 0, 0, 0, 0, 0, 0, 0 ] ],
        [ [ 6, 6, 6, 6, 6, 6, 8, 10, 12, 14, 16, 20, 24, 28, 32, 38, 46, 52, 60, 68, 58 ],
          [ 4, 4, 4, 6, 8, 10, 12, 14, 18, 24, 30, 40, 0, 0, 0, 0, 0, 0, 0, 0, 0 ] ],
        [ [ 6, 6, 6, 6, 6, 6, 8, 10, 12, 14, 16, 20, 24, 28, 32, 38, 46, 52, 60, 68, 58 ],
          [ 4, 4, 4, 6, 6, 8, 10, 14, 18, 26, 32, 42, 0, 0, 0, 0, 0, 0, 0, 0, 0 ] ],
        [ [ 6, 6, 6, 6, 6, 6, 8, 10, 12, 14, 16, 18, 22, 26, 32, 38, 46, 54, 62, 70, 76 ],
          [ 4, 4, 4, 6, 8, 10, 12, 14, 18, 24, 32, 44, 0, 0, 0, 0, 0, 0, 0, 0, 0 ] ],
        [ [ 4, 4, 4, 4, 4, 4, 6, 6, 8, 10, 12, 16, 20, 24, 30, 38, 46, 56, 68, 84, 102 ],
          [ 4, 4, 4, 4, 6, 8, 12, 16, 20, 26, 34, 42, 0, 0, 0, 0, 0, 0, 0, 0, 0 ] ],
        [ [ 4, 4, 4, 4, 4, 4, 6, 6, 8, 8, 10, 12, 16, 20, 24, 28, 34, 42, 50, 54, 76 ],
          [ 4, 4, 4, 4, 6, 8, 10, 12, 14, 18, 22, 30, 0, 0, 0, 0, 0, 0, 0, 0, 0 ] ],
        [ [ 4, 4, 4, 4, 4, 4, 6, 6, 6, 8, 10, 12, 16, 18, 22, 28, 34, 40, 46, 54, 54 ],
          [ 4, 4, 4, 4, 6, 6, 10, 12, 14, 16, 20, 26, 0, 0, 0, 0, 0, 0, 0, 0, 0 ] ],
    ];

    fn sfb_width(sf: u32, grsi: &GranuleSideInfo, sfb: usize) -> u8 {
        let findex = match sf {
            8000 => SpectralMap::FREQ_8KHZ,
            11025 => SpectralMap::FREQ_11_025KHZ,
            12000 => SpectralMap::FREQ_12KHZ,
            16000 => SpectralMap::FREQ_16KHZ,
            22050 => SpectralMap::FREQ_22_05KHZ,
            24000 => SpectralMap::FREQ_24KHZ,
            32000 => SpectralMap::FREQ_32KHZ,
            44100 => SpectralMap::FREQ_44_1KHZ,
            48000 => SpectralMap::FREQ_48KHZ,
            _ => unreachable!(),
        };
        let bindex = match grsi.block_type {
            BlockType::Long | BlockType::Start | BlockType::End => SpectralMap::BLOCK_LONG_INDEX,
            BlockType::Short => SpectralMap::BLOCK_SHORT_INDEX,
        };
        SpectralMap::SCALE_FACTOR_BANDS[findex][bindex][sfb]
    }

    fn new(sf: u32, grsi: &GranuleSideInfo) -> SpectralMap {
        let region0_max = grsi.region0_count as usize;
        let region1_max = grsi.region0_count as usize + grsi.region1_count as usize;
        let region2_max = grsi.big_values as usize * 2;

        let mut map = [(0, 0); 576];
        // TODO: Short blocks

        let mut sfb = 0;
        let mut sfb_threshold = Self::sfb_width(sf, grsi, 0) as usize;

        for l in 0..576 {
            if l > sfb_threshold {
                sfb_threshold += Self::sfb_width(sf, grsi, sfb) as usize;
                sfb += 1;
            }

            let region = match sfb {
                0..region0_max => Self::REGION0,
                region0_max..region1_max => Self::REGION1,
                _ => Self::REGION2_OR_OTHERS,
            };

            map[l] = (region, sfb as u8);
        }

        SpectralMap { map, grsi }
    }

    fn get_line_sfb(&self, l: usize) -> u8 {
        self.map[l].0
    }

    fn get_line_region_index(&self, l: usize) -> u8 {
        self.map[l].1
    }
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
    header: &MPEGHeader,
    grsi: &GranuleSideInfo,
    bits: usize,
    biter: &mut Biter<R, BUF_SIZE>,
) -> Result<GranuleSpectralValues, DecodeMainDataError> {
    let mut values = GranuleSpectralValues::default();
    let mut l: usize = 0;
    let mut consumed_bits = 0;
    let mut spectral_map = SpectralMap::new(header.sampling_frequency, grsi);

    /* big_values */
    while l < (grsi.big_values * 2) as usize {
        let region_index = spectral_map.get_line_region_index(l);
        let table_index = grsi.table_select[region_index];
        let table: &BigHuffmanTable = &BIG_TABLES[table_index];

        let mut decoder = BigHuffmanDecoder::new(table);
        let code = loop {
            match decoder.feed(biter.uimsbf(1)?) {
                BigHuffmanDecoderResult::Valid(data) => break data,
                BigHuffmanDecoderResult::InvalidCode => {
                    return Err(DecodeMainDataError::InvalidBigHuffmanCode);
                }
                BigHuffmanDecoderResult::NeedMoreBits => continue,
            }
        };

        let linbitsx: u32 = if code.x == 15 {
            biter.uimsbf(table.linbits as usize)?
        } else {
            0
        };
        let linbitsy: u32 = if code.y == 15 {
            biter.uimsbf(table.linbits as usize)?
        } else {
            0
        };

        let signx: i16 = if code.x != 0 {
            if (biter.uimsbf(1)? as bool) { 1 } else { -1 }
        } else {
            1
        };
        let signy: i16 = if code.y != 0 {
            if (biter.uimsbf(1)? as bool) { 1 } else { -1 }
        } else {
            1
        };

        let x: i16 = ((code.x as i16 & 15) << table.linbits | linbitsx as i16) * signx;
        let y: i16 = ((code.y as i16 & 15) << table.linbits | linbitsy as i16) * signy;

        values.0[l] = x;
        values.0[l + 1] = y;

        l += 2;
    }
    /* count1 */
    while consumed_bits < bits {
        l += 4;
    }
    /* rzero */
    while l < 576 {
        values.0[l] = 0;
        l += 1;
    }
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
            debug!(
                "huffman_bits={}, after huffman_bits = {}",
                huffman_bits,
                biter.bits_read()
            );
        }
    }

    Ok(MainData {
        scale_factors,
        spectral_values,
    })
}
