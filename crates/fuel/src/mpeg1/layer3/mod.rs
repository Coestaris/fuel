use crate::bite::{Biter, DEFAULT_BUF_SIZE};
use crate::header::{MPEGBitrate, MPEGHeader, MPEGMode};
use crate::mpeg1::layer3::main_data::{DecodeMainDataError, MainData, decode_main_data};
use crate::mpeg1::layer3::reservoir::{Reservoir, ReservoirError};
use crate::mpeg1::layer3::side_info::{DecodeSideInfoError, SideInfo, decode_side_info};
use crate::{Audio, Decoder, Frame, FuelError};
use log::debug;
use std::error::Error;
use std::io::{Cursor, Read};
use thiserror::Error;

mod main_data;
mod reservoir;
mod side_info;
mod huffman;

#[derive(Debug, Error)]
pub enum MPEG1Layer3DecoderNewError {}

#[derive(Debug, Error)]
pub enum MPEG1Layer3DecodeError {
    #[error(transparent)]
    SideInfo(#[from] DecodeSideInfoError),
    #[error("Unsupported free bitrate")]
    UnsupportedFreeBitrate,
    #[error("Invalid frame size")]
    InvalidFrameSize,
    #[error(transparent)]
    Reservoir(#[from] ReservoirError),
    #[error(transparent)]
    MainData(#[from] DecodeMainDataError),
}

fn side_info_size_bytes(header: &MPEGHeader) -> usize {
    match header.mode {
        MPEGMode::SingleChannel => 17,
        _ => 32,
    }
}

fn frame_size_bytes(header: &MPEGHeader) -> Result<usize, MPEG1Layer3DecodeError> {
    let bitrate_kbps = match header.bitrate {
        MPEGBitrate::Fixed(kbps) => kbps as usize,
        MPEGBitrate::Free => return Err(MPEG1Layer3DecodeError::UnsupportedFreeBitrate),
    };

    let padding = if header.padding_bit { 1 } else { 0 };

    Ok((144_000 * bitrate_kbps) / header.sampling_frequency as usize + padding)
}

fn decode_frame<R: Read, const BUF_SIZE: usize>(
    header: &MPEGHeader,
    mut biter: &mut Biter<R, BUF_SIZE>,
    reservoir: &mut Reservoir,
) -> Result<(SideInfo, MainData), MPEG1Layer3DecodeError> {
    debug!("after header bit pos = {}", biter.bits_read());
    let side_info = decode_side_info(&header, &mut biter)?;
    debug!("after side info bit pos = {}", biter.bits_read());

    debug_assert!(biter.is_aligned::<8>());

    let frame_size = frame_size_bytes(header)?;
    let crc_size = if header.crc_word.is_some() { 2 } else { 0 };
    let side_info_size = side_info_size_bytes(header);
    let main_data_bytes = frame_size
        .checked_sub(4 + crc_size + side_info_size)
        .ok_or(MPEG1Layer3DecodeError::InvalidFrameSize)?;

    let main_data_bytes =
        reservoir.bite_and_view(side_info.main_data_begin as usize, main_data_bytes, biter)?;
    let mut main_data_cursor = Cursor::new(main_data_bytes);
    let mut main_data_biter: Biter<&mut _, { DEFAULT_BUF_SIZE }> =
        Biter::new(&mut main_data_cursor);

    debug!("after main data pos = {}", biter.bits_read());

    let main_data = decode_main_data(
        header,
        &side_info,
        &mut main_data_biter,
        main_data_bytes.len() * 8,
    )?;

    Ok((side_info, main_data))
}

struct Mpeg1Layer3Decoder {
    reservoir: Reservoir,
}

impl Mpeg1Layer3Decoder {
    fn new() -> Self {
        Mpeg1Layer3Decoder {
            reservoir: Reservoir::new(),
        }
    }
}

impl<R: Read, const BUF_SIZE: usize> Decoder<R, BUF_SIZE> for Mpeg1Layer3Decoder {
    fn decode_samples(
        &mut self,
        header: &MPEGHeader,
        biter: &mut Biter<R, BUF_SIZE>,
    ) -> Result<Frame, Box<dyn Error>>
    where
        Self: Sized,
    {
        let (side_info, main_data) =
            decode_frame::<R, BUF_SIZE>(header, biter, &mut self.reservoir)?;

        debug!("Side info = {:?}", side_info);
        debug!("main_data = {:?}", main_data);

        Ok(Frame {
            sample_rate: header.sampling_frequency,
            samples: [0.0; 511],
            used: 0,
        })
    }
}

pub(crate) fn mpeg1_layer3_decoder_new<R: Read, const BUF_SIZE: usize>(
    _: &MPEGHeader,
) -> Result<Box<dyn Decoder<R, BUF_SIZE>>, MPEG1Layer3DecoderNewError> {
    Ok(Box::new(Mpeg1Layer3Decoder::new()))
}
