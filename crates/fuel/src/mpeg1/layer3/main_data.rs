use crate::bite::Biter;
use crate::header::MPEGHeader;
use crate::mpeg1::layer3::MPEG1Layer3ParseError;
use crate::mpeg1::layer3::side_info::SideInfo;
use std::io::Read;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MpegParseMainDataError {}

#[derive(Debug)]
pub struct MainData {}

pub(crate) fn parse_main_data<R: Read, const BUF_SIZE: usize>(
    header: &MPEGHeader,
    side_info: &SideInfo,
    main_data_biter: &mut Biter<R, BUF_SIZE>,
) -> Result<MainData, MPEG1Layer3ParseError> {
    Ok(MainData {})
}
