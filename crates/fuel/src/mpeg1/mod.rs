use crate::bite::Biter;
use crate::header::{MPEGHeader, MPEGLayer};
use crate::mpeg1::layer3::{MPEG1Layer3ParseError, parse_mpeg1_layer3};
use std::io::Read;
use thiserror::Error;

pub mod layer3;

#[derive(Debug, Error)]
pub enum MPEG1ParseError {
    #[error("Unsupported layer: {0}")]
    UnsupportedLayer(MPEGLayer),
    #[error(transparent)]
    Layer3Failed(#[from] MPEG1Layer3ParseError),
}

pub(crate) fn parse_mpeg1<R: Read, const BUF_SIZE: usize>(
    header: &MPEGHeader,
    biter: &mut Biter<R, BUF_SIZE>,
) -> Result<Vec<f32>, MPEG1ParseError> {
    Ok(match header.layer {
        MPEGLayer::LayerI => Err(MPEG1ParseError::UnsupportedLayer(header.layer))?,
        MPEGLayer::LayerII => Err(MPEG1ParseError::UnsupportedLayer(header.layer))?,
        MPEGLayer::LayerIII => parse_mpeg1_layer3(header, biter)?,
    })
}
