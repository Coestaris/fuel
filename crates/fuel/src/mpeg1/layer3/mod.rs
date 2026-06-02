use crate::bite::Biter;
use crate::header::MPEGHeader;
use crate::mpeg1::layer3::main_data::{MpegParseMainDataError, parse_main_data};
use crate::mpeg1::layer3::side_info::{MpegParseSideInfoError, parse_side_info};
use log::debug;
use std::io::Read;
use thiserror::Error;

mod main_data;
mod side_info;

#[derive(Debug, Error)]
pub enum MPEG1Layer3ParseError {
    #[error(transparent)]
    SideInfo(#[from] MpegParseSideInfoError),
    #[error(transparent)]
    MainData(#[from] MpegParseMainDataError),
}

pub(crate) fn parse_mpeg1_layer3<R: Read, const BUF_SIZE: usize>(
    header: &MPEGHeader,
    mut biter: &mut Biter<R, BUF_SIZE>,
) -> Result<Vec<f32>, MPEG1Layer3ParseError> {
    debug!("after header bit pos = {}", biter.bits_read());
    let (side_info, main_data_begin) = parse_side_info(&header, &mut biter)?;
    debug!("after side info bit pos = {}", biter.bits_read());

    debug!("Main data begin {:#?}", main_data_begin);
    debug!("Size info {:#?}", side_info);

    let main_data = parse_main_data(&header, &side_info, main_data_begin, &mut biter)?;
    debug!("after main data bit pos = {}", biter.bits_read());
    debug!("Main data {:#?}", main_data);

    Ok(vec![])
}
