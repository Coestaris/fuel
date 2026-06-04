use std::io;
use std::io::{Read, Seek, SeekFrom};
use thiserror::Error;

pub(crate) const DEFAULT_BUF_SIZE: usize = 1024;

#[derive(Debug, Error)]
pub enum BiterError {
    #[error("IO error: {0}")]
    IO(#[from] io::Error),
    #[error("Unexpected end of file")]
    EOF,
    #[error("Unaligned buffer")]
    Unaligned,
    #[error("Cannot read more than {0} at once, got: {1}")]
    TooManyBitsToRead(usize, usize),
    #[error("Output buffer is too small: {0} < {1}")]
    SmallInputBuffer(usize, usize),
}

pub struct Biter<R, const BUF_SIZE: usize = DEFAULT_BUF_SIZE> {
    stream: R,

    // Byte-level buffer for fewer syscalls / Read calls.
    byte_buf: [u8; BUF_SIZE],
    byte_pos: usize,
    byte_len: usize,

    // Bit reservoir.
    //
    // Invariant:
    // - unread bits are stored in the lowest `bit_len` bits;
    // - the next bit to read is the highest bit among those `bit_len` bits.
    //
    // Example:
    //   bit_buf low bits: 101100
    //   bit_len: 6
    //   next bit: leftmost `1`
    bit_buf: u64,
    bit_len: u8,
}

pub trait FromBSLBF: Sized {
    fn from_bits(s: u32) -> Self;
}

pub trait FromUIMSBF: Sized {
    fn from_bits(s: u32) -> Self;
}

impl<R: Read, const BUF_SIZE: usize> Biter<R, BUF_SIZE> {
    pub fn new(stream: R) -> Self {
        assert!(BUF_SIZE > 0);

        Self {
            stream,
            byte_buf: [0; BUF_SIZE],
            byte_pos: 0,
            byte_len: 0,
            bit_buf: 0,
            bit_len: 0,
        }
    }

    #[inline(always)]
    fn next_byte(&mut self) -> Result<u8, BiterError> {
        if self.byte_pos == self.byte_len {
            self.byte_len = self.stream.read(&mut self.byte_buf)?;
            self.byte_pos = 0;

            if self.byte_len == 0 {
                Err(BiterError::EOF)?
            }
        }

        let byte = self.byte_buf[self.byte_pos];
        self.byte_pos += 1;
        Ok(byte)
    }

    #[inline(always)]
    fn ensure_bits(&mut self, bits: usize) -> Result<(), BiterError> {
        debug_assert!(bits <= 32);

        while usize::from(self.bit_len) < bits {
            let byte = u64::from(self.next_byte()?);

            self.bit_buf = (self.bit_buf << 8) | byte;
            self.bit_len += 8;
        }

        Ok(())
    }

    #[inline(always)]
    pub fn read_bits_u32(&mut self, bits: usize) -> Result<u32, BiterError> {
        if bits > 32 {
            Err(BiterError::TooManyBitsToRead(bits, 32))?
        }

        if bits == 0 {
            return Ok(0);
        }

        // Fast path for byte-aligned reads.
        // Very useful for fields like 8/16/24/32 bits.
        if self.bit_len == 0 && bits % 8 == 0 {
            let mut value = 0u32;

            for _ in 0..(bits / 8) {
                value = (value << 8) | u32::from(self.next_byte()?);
            }

            return Ok(value);
        }

        self.ensure_bits(bits)?;

        let shift = usize::from(self.bit_len) - bits;

        let mask = if bits == 32 {
            u64::from(u32::MAX)
        } else {
            (1u64 << bits) - 1
        };

        let value = ((self.bit_buf >> shift) & mask) as u32;

        self.bit_len -= bits as u8;

        if self.bit_len == 0 {
            self.bit_buf = 0;
        } else {
            self.bit_buf &= (1u64 << usize::from(self.bit_len)) - 1;
        }

        Ok(value)
    }

    /// Bitstring, left bit first.
    ///
    /// For raw bits `1000_0111`, this returns raw value `0x87`.
    #[inline(always)]
    pub fn bslbf<T: FromBSLBF>(&mut self, bits: usize) -> Result<T, BiterError> {
        let raw = self.read_bits_u32(bits)?;
        Ok(T::from_bits(raw))
    }

    /// Unsigned integer, most significant bit first.
    ///
    /// For raw bits `1000_0111`, this returns numeric value `135`.
    #[inline(always)]
    pub fn uimsbf<T: FromUIMSBF>(&mut self, bits: usize) -> Result<T, BiterError> {
        let raw = self.read_bits_u32(bits)?;
        Ok(T::from_bits(raw))
    }

    /// Discard bits until next byte boundary.
    ///
    /// Important for formats where some fields are byte-aligned after bit fields.
    #[inline(always)]
    pub fn byte_align(&mut self) {
        let rem = self.bit_len % 8;

        if rem == 0 {
            return;
        }

        self.bit_len -= rem;

        if self.bit_len == 0 {
            self.bit_buf = 0;
        } else {
            self.bit_buf &= (1u64 << usize::from(self.bit_len)) - 1;
        }
    }

    #[inline(always)]
    pub fn is_aligned<const S: usize>(&self) -> bool {
        assert!(S > 0);
        self.bit_len as usize % S == 0
    }

    #[inline(always)]
    pub fn into_inner(self) -> R {
        self.stream
    }

    #[inline(always)]
    pub fn bits_read(&self) -> u64 {
        let byte_bits = (self.byte_pos as u64) * 8;
        let bit_bits = u64::from(self.bit_len);
        byte_bits + bit_bits
    }

    fn iterate_bits<F>(&mut self, mut bits: usize, mut f: F) -> Result<(), BiterError>
    where
        F: FnMut(u8),
    {
        while bits > 0 {
            let n = bits.min(8);
            f(self.uimsbf(n)?);
            bits -= n;
        }

        Ok(())
    }

    pub fn skip_bits(&mut self, bits: usize) -> Result<(), BiterError> {
        self.iterate_bits(bits, |_| {})
    }

    pub fn read_bits(&mut self, bits: usize, out: &mut [u8]) -> Result<(), BiterError> {
        let req = bits.div_ceil(8);
        if out.len() < req {
            Err(BiterError::SmallInputBuffer(req, out.len()))?
        }

        let mut idx = 0;
        self.iterate_bits(bits, |byte| {
            out[idx] = byte;
            idx += 1;
        })
    }

    pub fn read_aligned_bytes(&mut self, out: &mut [u8]) -> Result<(), BiterError> {
        if !self.is_aligned::<8>() {
            Err(BiterError::Unaligned)?
        }

        for byte in out {
            *byte = self.uimsbf(8)?;
        }

        Ok(())
    }
}

impl<R: Read + Seek, const BUF_SIZE: usize> Biter<R, BUF_SIZE> {
    /// Absolute byte seek.
    ///
    /// After seeking, all internal buffers are invalidated.
    /// I would avoid `SeekFrom::Current` here unless you carefully track logical bit position.
    pub fn seek_to_byte(&mut self, byte_offset: u64) -> io::Result<u64> {
        self.byte_pos = 0;
        self.byte_len = 0;
        self.bit_buf = 0;
        self.bit_len = 0;

        self.stream.seek(SeekFrom::Start(byte_offset))
    }
}

macro_rules! impl_from_bits {
    ($trait_name:ident for $($ty:ty),+ $(,)?) => {
        $(
            impl $trait_name for $ty {
                #[inline(always)]
                fn from_bits(bits: u32) -> Self {
                    bits as $ty
                }
            }
        )+
    };
}

impl_from_bits!(FromUIMSBF for u8, u16, u32, u64, usize);
impl_from_bits!(FromBSLBF for u8, u16, u32, u64, usize);

impl FromUIMSBF for bool {
    #[inline(always)]
    fn from_bits(bits: u32) -> Self {
        bits != 0
    }
}

impl FromBSLBF for bool {
    #[inline(always)]
    fn from_bits(bits: u32) -> Self {
        bits != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bite;

    #[test]
    pub fn test_bits() {
        let data = [0b1010_1010, 0b1100_1100];
        let mut biter: Biter<&_, DEFAULT_BUF_SIZE> = Biter::new(&data[..]);

        assert_eq!(biter.read_bits_u32(4).unwrap(), 0b1010);
        assert_eq!(biter.read_bits_u32(4).unwrap(), 0b1010);
        assert_eq!(biter.read_bits_u32(8).unwrap(), 0b1100_1100);
    }
}
