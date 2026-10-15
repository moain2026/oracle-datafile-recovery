//! Bounds-checked byte access with explicit endianness.
//!
//! Oracle datafiles are written in the byte order of the platform that created
//! them (Linux/Windows x86: little; AIX/SPARC/HP-UX: big). The order is decided
//! once per file (by the header probe) and carried in [`ByteOrder`].

use crate::error::{DecodeError, DecodeResult};
use serde::{Deserialize, Serialize};

/// Byte order of a datafile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ByteOrder {
    /// Little-endian (x86/x64: Windows, Linux).
    Little,
    /// Big-endian (AIX, Solaris SPARC, HP-UX).
    Big,
}

macro_rules! read_int {
    ($name:ident, $ty:ty, $n:literal) => {
        #[doc = concat!("Read a `", stringify!($ty), "` at `offset` in the given byte order.")]
        ///
        /// # Errors
        /// [`DecodeError::Truncated`] if the buffer is too short.
        pub fn $name(buf: &[u8], offset: usize, order: ByteOrder) -> DecodeResult<$ty> {
            let arr: [u8; $n] = array_at(buf, offset)?;
            Ok(match order {
                ByteOrder::Little => <$ty>::from_le_bytes(arr),
                ByteOrder::Big => <$ty>::from_be_bytes(arr),
            })
        }
    };
}

read_int!(read_u16, u16, 2);
read_int!(read_u32, u32, 4);
read_int!(read_u64, u64, 8);

/// Read a single byte at `offset`.
///
/// # Errors
/// [`DecodeError::Truncated`] if `offset` is out of bounds.
pub fn read_u8(buf: &[u8], offset: usize) -> DecodeResult<u8> {
    buf.get(offset).copied().ok_or(DecodeError::Truncated {
        offset,
        need: 1,
        len: buf.len(),
    })
}

/// Borrow `len` bytes starting at `offset`.
///
/// # Errors
/// [`DecodeError::Truncated`] if the range is out of bounds (including overflow).
pub fn slice_at(buf: &[u8], offset: usize, len: usize) -> DecodeResult<&[u8]> {
    offset
        .checked_add(len)
        .and_then(|end| buf.get(offset..end))
        .ok_or(DecodeError::Truncated {
            offset,
            need: len,
            len: buf.len(),
        })
}

fn array_at<const N: usize>(buf: &[u8], offset: usize) -> DecodeResult<[u8; N]> {
    let s = slice_at(buf, offset, N)?;
    // `slice_at` guarantees `s.len() == N`.
    s.try_into().map_err(|_| DecodeError::Truncated {
        offset,
        need: N,
        len: buf.len(),
    })
}

/// A forward-reading cursor over an untrusted buffer.
#[derive(Debug, Clone)]
pub struct Cursor<'a> {
    buf: &'a [u8],
    pos: usize,
    order: ByteOrder,
}

impl<'a> Cursor<'a> {
    /// Create a cursor at position 0.
    pub const fn new(buf: &'a [u8], order: ByteOrder) -> Self {
        Self { buf, pos: 0, order }
    }

    /// Current position.
    pub const fn position(&self) -> usize {
        self.pos
    }

    /// Byte order used for multi-byte reads.
    pub const fn order(&self) -> ByteOrder {
        self.order
    }

    /// Bytes left after the current position.
    pub const fn remaining(&self) -> usize {
        self.buf.len().saturating_sub(self.pos)
    }

    /// Move to an absolute position (may equal the buffer length).
    ///
    /// # Errors
    /// [`DecodeError::Truncated`] if `pos` is past the end.
    pub fn seek(&mut self, pos: usize) -> DecodeResult<()> {
        if pos > self.buf.len() {
            return Err(DecodeError::Truncated {
                offset: pos,
                need: 0,
                len: self.buf.len(),
            });
        }
        self.pos = pos;
        Ok(())
    }

    /// Take the next `n` bytes.
    ///
    /// # Errors
    /// [`DecodeError::Truncated`] if fewer than `n` bytes remain.
    pub fn take(&mut self, n: usize) -> DecodeResult<&'a [u8]> {
        let s = slice_at(self.buf, self.pos, n)?;
        self.pos = self.pos.saturating_add(n);
        Ok(s)
    }

    /// Skip `n` bytes.
    ///
    /// # Errors
    /// [`DecodeError::Truncated`] if fewer than `n` bytes remain.
    pub fn skip(&mut self, n: usize) -> DecodeResult<()> {
        self.take(n).map(|_| ())
    }

    /// Read one byte.
    ///
    /// # Errors
    /// [`DecodeError::Truncated`] at end of buffer.
    pub fn u8(&mut self) -> DecodeResult<u8> {
        let v = read_u8(self.buf, self.pos)?;
        self.pos = self.pos.saturating_add(1);
        Ok(v)
    }

    /// Read a `u16` in the cursor's byte order.
    ///
    /// # Errors
    /// [`DecodeError::Truncated`] if fewer than 2 bytes remain.
    pub fn u16(&mut self) -> DecodeResult<u16> {
        let v = read_u16(self.buf, self.pos, self.order)?;
        self.pos = self.pos.saturating_add(2);
        Ok(v)
    }

    /// Read a `u32` in the cursor's byte order.
    ///
    /// # Errors
    /// [`DecodeError::Truncated`] if fewer than 4 bytes remain.
    pub fn u32(&mut self) -> DecodeResult<u32> {
        let v = read_u32(self.buf, self.pos, self.order)?;
        self.pos = self.pos.saturating_add(4);
        Ok(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_both_orders() {
        let b = [0x01, 0x02, 0x03, 0x04];
        assert_eq!(read_u16(&b, 0, ByteOrder::Little), Ok(0x0201));
        assert_eq!(read_u16(&b, 0, ByteOrder::Big), Ok(0x0102));
        assert_eq!(read_u32(&b, 0, ByteOrder::Little), Ok(0x0403_0201));
        assert_eq!(read_u32(&b, 0, ByteOrder::Big), Ok(0x0102_0304));
    }

    #[test]
    fn out_of_bounds_is_error_not_panic() {
        let b = [0u8; 3];
        assert!(read_u32(&b, 0, ByteOrder::Little).is_err());
        assert!(read_u16(&b, usize::MAX, ByteOrder::Big).is_err());
        assert!(slice_at(&b, usize::MAX, 2).is_err());
        let mut c = Cursor::new(&b, ByteOrder::Little);
        assert!(c.take(4).is_err());
        assert_eq!(c.position(), 0, "failed read must not advance");
        assert!(c.seek(4).is_err());
    }
}
