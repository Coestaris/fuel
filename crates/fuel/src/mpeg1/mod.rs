use std::io::Read;
use crate::Decoder;
use crate::header::{MPEGHeader, MPEGLayer};
use crate::mpeg1::layer3::{MPEG1Layer3DecoderNewError, mpeg1_layer3_decoder_new};
use thiserror::Error;

pub mod layer3;

#[derive(Debug, Error)]
pub enum MPEG1DecoderNewError {
    #[error("Unsupported layer: {0}")]
    UnsupportedLayer(MPEGLayer),
    #[error("Failed to create MPEG1 LayerIII decoder: {0}")]
    FailedToCreateMPEG1Layer3Decoder(#[from] MPEG1Layer3DecoderNewError),
}

pub(crate) fn mpeg1_decoder_new<R: Read, const BUF_SIZE: usize>(
    header: &MPEGHeader,
) -> Result<Box<dyn Decoder<R, BUF_SIZE>>, MPEG1DecoderNewError> {
    Ok(match header.layer {
        MPEGLayer::LayerI => Err(MPEG1DecoderNewError::UnsupportedLayer(header.layer))?,
        MPEGLayer::LayerII => Err(MPEG1DecoderNewError::UnsupportedLayer(header.layer))?,
        MPEGLayer::LayerIII => mpeg1_layer3_decoder_new::<R, BUF_SIZE>(header)?,
    })
}
