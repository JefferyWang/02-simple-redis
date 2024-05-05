use super::{calc_total_length, parse_length_isize, CRLF_LEN};
use crate::{RespDecode, RespEncode, RespError, RespFrame, BUF_CAP};
use bytes::{Buf, BytesMut};
use lazy_static::lazy_static;
use std::ops::Deref;

lazy_static! {
    static ref EMPTY_VEC_RESPFRAME: Vec<RespFrame> = Vec::new();
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct RespArray(pub(crate) Option<Vec<RespFrame>>);

impl RespArray {
    pub fn new(s: impl Into<Vec<RespFrame>>) -> Self {
        Self(Some(s.into()))
    }

    pub fn null() -> Self {
        Self(None)
    }
}

// Arrays: "*<number-of-elements>\r\n<element-1>...<element-n>"
// Null arrays: "*-1\r\n"
impl RespEncode for RespArray {
    fn encode(self) -> Vec<u8> {
        match self.0 {
            Some(frames) => {
                let mut buf = Vec::with_capacity(BUF_CAP);
                buf.extend_from_slice(format!("*{}\r\n", frames.len()).as_bytes());
                for frame in frames {
                    buf.extend_from_slice(&frame.encode());
                }
                buf
            }
            None => b"*-1\r\n".to_vec(),
        }
    }
}

// - array: "*<number-of-elements>\r\n<element-1>...<element-n>"
// - "*2\r\n$3\r\nget\r\n$5\r\nhello\r\n"
// FIXME: need to handle incomplete
impl RespDecode for RespArray {
    const PREFIX: &'static str = "*";

    fn decode(buf: &mut BytesMut) -> Result<Self, RespError> {
        let (end, len) = parse_length_isize(buf, Self::PREFIX)?;
        if len == -1 {
            return Ok(RespArray::null());
        }
        let len = len as usize;
        let total_len = calc_total_length(buf, end, len, Self::PREFIX)?;

        if buf.len() < total_len {
            return Err(RespError::NotComplete);
        }

        buf.advance(end + CRLF_LEN);

        let mut frames = Vec::with_capacity(len);
        for _ in 0..len {
            frames.push(RespFrame::decode(buf)?);
        }
        Ok(RespArray::new(frames))
    }

    fn expect_length(buf: &[u8]) -> Result<usize, RespError> {
        let (end, len) = parse_length_isize(buf, Self::PREFIX)?;
        if len == -1 {
            return Ok(end + CRLF_LEN);
        }
        let len = len as usize;
        calc_total_length(buf, end, len, Self::PREFIX)
    }
}

impl Deref for RespArray {
    type Target = Vec<RespFrame>;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref().unwrap_or(&EMPTY_VEC_RESPFRAME)
    }
}

#[cfg(test)]
mod tests {
    use crate::BulkString;

    use super::*;
    use anyhow::Result;

    #[test]
    fn test_array_encode() {
        let frame: RespFrame = RespArray::new(vec![
            BulkString::new("set".to_string()).into(),
            BulkString::new("hello".to_string()).into(),
            BulkString::new("world".to_string()).into(),
        ])
        .into();

        assert_eq!(
            frame.encode(),
            b"*3\r\n$3\r\nset\r\n$5\r\nhello\r\n$5\r\nworld\r\n"
        );
    }

    #[test]
    fn test_array_decode() -> Result<()> {
        let mut buf = BytesMut::new();
        buf.extend_from_slice(b"*2\r\n$3\r\nset\r\n$5\r\nhello\r\n");

        let frame = RespArray::decode(&mut buf)?;
        assert_eq!(frame, RespArray::new([b"set".into(), b"hello".into()]));

        buf.extend_from_slice(b"*2\r\n$3\r\nset\r\n");
        let ret = RespArray::decode(&mut buf);
        assert_eq!(ret.unwrap_err(), RespError::NotComplete);

        buf.extend_from_slice(b"$5\r\nhello\r\n");
        let frame = RespArray::decode(&mut buf)?;
        assert_eq!(frame, RespArray::new([b"set".into(), b"hello".into()]));

        Ok(())
    }

    #[test]
    fn test_null_array_encode() {
        let frame: RespFrame = RespArray::null().into();
        assert_eq!(frame.encode(), b"*-1\r\n");
    }

    #[test]
    fn test_null_array_decode() -> Result<()> {
        let mut buf = BytesMut::new();
        buf.extend_from_slice(b"*-1\r\n");

        let frame = RespArray::decode(&mut buf)?;
        assert_eq!(frame, RespArray::null());

        Ok(())
    }
}
