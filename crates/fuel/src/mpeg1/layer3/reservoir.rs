use std::io::{self, Read};
use crate::bite::Biter;

const RESERVOIR_BACKPTR_MAX: usize = 511;

const RESERVOIR_CAP: usize = 2048;

#[derive(Debug, thiserror::Error)]
pub enum ReservoirError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("main_data_begin is too large: {main_data_begin}, available: {available}")]
    Underflow {
        main_data_begin: usize,
        available: usize,
    },
    #[error("main data view is too large: {needed}, capacity: {capacity}")]
    TooLarge {
        needed: usize,
        capacity: usize,
    },
    #[error("bit reader is not byte-aligned")]
    NotByteAligned,
}

pub struct Reservoir {
    // Mirrored ring:
    //  buffer[i]
    //  buffer[i + RESERVOIR_CAP]
    buffer: [u8; RESERVOIR_CAP * 2],
    position: usize,
    used: usize,
}

impl Reservoir {
    pub fn new() -> Self {
        Self {
            buffer: [0; RESERVOIR_CAP * 2],
            position: 0,
            used: 0,
        }
    }

    // Reads current frame main_data into the reservoir and returns a contiguous
    // logical view:
    // [main_data_begin bytes from previous frames][current frame main_data]
    pub fn bite_and_view<R: Read, const BUF_SIZE: usize>(
        &mut self,
        main_data_begin: usize,
        main_data_bytes: usize,
        biter: &mut Biter<R, BUF_SIZE>,
    ) -> Result<&[u8], ReservoirError> {
        if !biter.is_aligned::<8>() {
            return Err(ReservoirError::NotByteAligned);
        }

        if main_data_begin > RESERVOIR_BACKPTR_MAX {
            return Err(ReservoirError::TooLarge {
                needed: main_data_begin,
                capacity: RESERVOIR_BACKPTR_MAX,
            });
        }

        if main_data_begin > self.used {
            return Err(ReservoirError::Underflow {
                main_data_begin,
                available: self.used,
            });
        }

        let view_len = main_data_begin + main_data_bytes;

        if view_len > RESERVOIR_CAP {
            return Err(ReservoirError::TooLarge {
                needed: view_len,
                capacity: RESERVOIR_CAP,
            });
        }

        // Logical view starts `main_data_begin` bytes before current write position.
        let view_start = (self.position + RESERVOIR_CAP - main_data_begin) % RESERVOIR_CAP;

        self.read_into_ring(main_data_bytes, biter)?;

        // Because the buffer is mirrored, this is contiguous even if the logical
        // region wraps around RESERVOIR_CAP.
        Ok(&self.buffer[view_start..view_start + view_len])
    }

    fn read_into_ring<R: Read, const BUF_SIZE: usize>(
        &mut self,
        mut len: usize,
        biter: &mut Biter<R, BUF_SIZE>,
    ) -> Result<(), ReservoirError> {
        while len > 0 {
            let first_chunk = (RESERVOIR_CAP - self.position).min(len);

            biter.read_aligned_bytes(
                &mut self.buffer[self.position..self.position + first_chunk],
            )?;

            // Mirror written bytes.
            let mirror_src_start = self.position;
            let mirror_src_end = self.position + first_chunk;
            let mirror_dst_start = self.position + RESERVOIR_CAP;

            self.buffer.copy_within(
                mirror_src_start..mirror_src_end,
                mirror_dst_start,
            );

            self.position = (self.position + first_chunk) % RESERVOIR_CAP;
            self.used = (self.used + first_chunk).min(RESERVOIR_CAP);

            len -= first_chunk;
        }

        Ok(())
    }
}