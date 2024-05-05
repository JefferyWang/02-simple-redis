use super::{parse_length_isize, CRLF_LEN};
use crate::{RespDecode, RespEncode, RespError};
use bytes::{Buf, BytesMut};
use lazy_static::lazy_static;
use std::ops::Deref;

lazy_static! {
    static ref EMPTY_VEC_U8: Vec<u8> = Vec::new();
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd)]
pub struct BulkString(pub(crate) Option<Vec<u8>>);

impl BulkString {
    pub fn new(s: impl Into<Vec<u8>>) -> Self {
        Self(Some(s.into()))
    }

    pub fn null() -> Self {
        Self(None)
    }

    pub fn get_data(&self) -> Result<Vec<u8>, RespError> {
        match &self.0 {
            Some(data) => Ok(data.clone()),
            None => Err(RespError::InvalidFrame("BulkString is None".to_string())),
        }
    }
}

// Bulk strings: "$<length>\r\n<data>\r\n"
// Null bulk strings: "$-1\r\n"
impl RespEncode for BulkString {
    fn encode(self) -> Vec<u8> {
        match self.0 {
            Some(data) => {
                let mut buf = Vec::with_capacity(data.len() + 16);
                buf.extend_from_slice(format!("${}\r\n", data.len()).as_bytes());
                buf.extend_from_slice(&data);
                buf.extend_from_slice(b"\r\n");
                buf
            }
            None => b"$-1\r\n".to_vec(),
        }
    }
}

// Bulk strings: "$<length>\r\n<data>\r\n"
impl RespDecode for BulkString {
    const PREFIX: &'static str = "$";

    fn decode(buf: &mut BytesMut) -> Result<Self, RespError> {
        let (end, len) = parse_length_isize(buf, Self::PREFIX)?;
        if len == -1 {
            return Ok(BulkString::null());
        }
        let len = len as usize;
        let remained = &buf[end + CRLF_LEN..];
        if remained.len() < len + CRLF_LEN {
            return Err(RespError::NotComplete);
        }

        buf.advance(end + CRLF_LEN);

        let data = buf.split_to(len + CRLF_LEN);
        Ok(BulkString::new(data[..len].to_vec()))
    }

    fn expect_length(buf: &[u8]) -> Result<usize, RespError> {
        let (end, len) = parse_length_isize(buf, Self::PREFIX)?;
        if len == -1 {
            return Ok(end + CRLF_LEN);
        }
        let len = len as usize;
        Ok(end + CRLF_LEN + len + CRLF_LEN)
    }
}

impl From<&str> for BulkString {
    fn from(s: &str) -> Self {
        BulkString(Some(s.as_bytes().to_vec()))
    }
}

impl From<String> for BulkString {
    fn from(s: String) -> Self {
        BulkString(Some(s.as_bytes().to_vec()))
    }
}

impl From<&[u8]> for BulkString {
    fn from(s: &[u8]) -> Self {
        BulkString(Some(s.to_vec()))
    }
}

impl<const N: usize> From<&[u8; N]> for BulkString {
    fn from(s: &[u8; N]) -> Self {
        BulkString(Some(s.to_vec()))
    }
}

impl AsRef<[u8]> for BulkString {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref().unwrap_or(&EMPTY_VEC_U8)
    }
}

impl Deref for BulkString {
    type Target = Vec<u8>;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref().unwrap_or(&EMPTY_VEC_U8)
    }
}

#[cfg(test)]
mod tests {
    use crate::RespFrame;

    use super::*;
    use anyhow::Result;

    #[test]
    fn test_bulk_string_encode() {
        let frame: RespFrame = BulkString::new(b"Hello, world!".to_vec()).into();
        assert_eq!(frame.encode(), b"$13\r\nHello, world!\r\n");
    }

    #[test]
    fn test_null_bulk_string_encode() {
        let frame: RespFrame = BulkString::null().into();
        assert_eq!(frame.encode(), b"$-1\r\n");
    }

    #[test]
    fn test_bulk_string_decode() -> Result<()> {
        let mut buf = BytesMut::new();
        buf.extend_from_slice(b"$5\r\nhello\r\n");

        let frame = BulkString::decode(&mut buf)?;
        assert_eq!(frame, BulkString::new(b"hello"));

        buf.extend_from_slice(b"$5\r\nhello");
        let ret = BulkString::decode(&mut buf);
        assert_eq!(ret.unwrap_err(), RespError::NotComplete);

        buf.extend_from_slice(b"\r\n");
        let frame = BulkString::decode(&mut buf)?;
        assert_eq!(frame, BulkString::new(b"hello"));

        Ok(())
    }

    #[test]
    fn test_null_bulk_string_decode() -> Result<()> {
        let mut buf = BytesMut::new();
        buf.extend_from_slice(b"$-1\r\n");

        let frame = BulkString::decode(&mut buf)?;
        assert_eq!(frame, BulkString::null());

        Ok(())
    }
}
