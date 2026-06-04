use std::io;
use std::io::{ErrorKind, Read, Seek, SeekFrom};

pub(crate) const DEFAULT_BUF_SIZE: usize = 1024;

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
    fn next_byte(&mut self) -> io::Result<u8> {
        if self.byte_pos == self.byte_len {
            self.byte_len = self.stream.read(&mut self.byte_buf)?;
            self.byte_pos = 0;

            if self.byte_len == 0 {
                return Err(io::Error::new(
                    ErrorKind::UnexpectedEof,
                    "unexpected EOF while reading bitstream",
                ));
            }
        }

        let byte = self.byte_buf[self.byte_pos];
        self.byte_pos += 1;
        Ok(byte)
    }

    #[inline(always)]
    fn ensure_bits(&mut self, bits: usize) -> io::Result<()> {
        debug_assert!(bits <= 32);

        while usize::from(self.bit_len) < bits {
            let byte = u64::from(self.next_byte()?);

            self.bit_buf = (self.bit_buf << 8) | byte;
            self.bit_len += 8;
        }

        Ok(())
    }

    #[inline(always)]
    pub fn read_bits_u32(&mut self, bits: usize) -> io::Result<u32> {
        if bits > 32 {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "cannot read more than 32 bits into u32",
            ));
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
    pub fn bslbf<T: FromBSLBF>(&mut self, bits: usize) -> io::Result<T> {
        let raw = self.read_bits_u32(bits)?;
        Ok(T::from_bits(raw))
    }

    /// Unsigned integer, most significant bit first.
    ///
    /// For raw bits `1000_0111`, this returns numeric value `135`.
    #[inline(always)]
    pub fn uimsbf<T: FromUIMSBF>(&mut self, bits: usize) -> io::Result<T> {
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

    pub fn read_aligned_bytes(&mut self, out: &mut [u8]) -> io::Result<()> {
        if !self.is_aligned::<8>() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "bit reader is not byte-aligned",
            ));
        }

        self.stream.read_exact(out)?;
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
