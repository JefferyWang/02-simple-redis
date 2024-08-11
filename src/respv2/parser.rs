use std::collections::BTreeMap;
use std::vec;

use winnow::ascii::{digit1, float};
use winnow::combinator::{alt, dispatch, fail, opt, preceded, terminated};
use winnow::error::{ContextError, ErrMode};
use winnow::token::{any, take, take_until};
use winnow::{PResult, Parser};

use crate::{
    BulkString, RespArray, RespError, RespFrame, RespMap, RespNull, SimpleError, SimpleString,
};

const CRLF: &[u8] = b"\r\n";

pub fn parse_frame_length(input: &[u8]) -> Result<usize, RespError> {
    let target = &mut (&*input);
    let ret = parse_frame_len(target);
    match ret {
        Ok(_) => {
            let start = input.as_ptr() as usize;
            let end = (*target).as_ptr() as usize;
            let len = end - start;
            Ok(len)
        }
        Err(_) => Err(RespError::NotComplete),
    }
}

fn parse_frame_len(input: &mut &[u8]) -> PResult<()> {
    let mut simple_parser = terminated(take_until(0.., CRLF), CRLF).value(());
    dispatch! {
        any;
        b'+' => simple_parser,
        b'-' => simple_parser,
        b':' => simple_parser,
        b'$' => bulk_string_len,
        b'*' => array_len,
        b'_' => simple_parser,
        b'#' => simple_parser,
        b',' => simple_parser,
        b'%' => map_len,
        // b'~' => set,
        _v => fail::<_,_,_>,
    }
    .parse_next(input)
}

pub fn parse_frame(input: &mut &[u8]) -> PResult<RespFrame> {
    // frame type has been processed
    dispatch! {
        any;
        b'+' => simple_string.map(RespFrame::SimpleString),
        b'-' => error.map(RespFrame::Error),
        b':' => integer.map(RespFrame::Integer),
        b'$' => alt((null_bulk_string.map(RespFrame::BulkString), bulk_string.map(RespFrame::BulkString))),
        b'*' => alt((null_array.map(RespFrame::Array), array.map(RespFrame::Array))),
        b'_' => null.map(RespFrame::Null),
        b'#' => boolean.map(RespFrame::Boolean),
        b',' => double.map(RespFrame::Double),
        b'%' => map.map(RespFrame::Map),
        // b'~' => set,
        _v => fail::<_,_,_>,
    }
    .parse_next(input)
}

// Null: "_\r\n"
fn null(input: &mut &[u8]) -> PResult<RespNull> {
    CRLF.value(RespNull).parse_next(input)
}

// - simple string: "OK\r\n"
fn simple_string(input: &mut &[u8]) -> PResult<SimpleString> {
    parse_string.map(SimpleString).parse_next(input)
}

fn error(input: &mut &[u8]) -> PResult<SimpleError> {
    parse_string.map(SimpleError).parse_next(input)
}

// - integer: ":1000\r\n"
fn integer(input: &mut &[u8]) -> PResult<i64> {
    let sign = opt(alt((b'+', b'-'))).parse_next(input)?.unwrap_or(b'+');
    let sign: i64 = if sign == b'+' { 1 } else { -1 };
    let v: i64 = terminated(digit1.parse_to(), CRLF).parse_next(input)?;
    Ok(sign * v)
}

// Null bulk strings: "$-1\r\n"
fn null_bulk_string(input: &mut &[u8]) -> PResult<BulkString> {
    "-1\r\n".value(BulkString::null()).parse_next(input)
}

// - bulk string: "$6\r\nfoobar\r\n"
#[allow(clippy::comparison_chain)]
fn bulk_string(input: &mut &[u8]) -> PResult<BulkString> {
    let len: i64 = integer.parse_next(input)?;
    if len == 0 {
        return Ok(BulkString::new(vec![]));
    } else if len < 0 {
        return Err(err_cut("bulk string length must be non-negative"));
    }
    let len = len as usize;
    let data = terminated(take(len), CRLF)
        .map(|s: &[u8]| s.to_vec())
        .parse_next(input)?;
    Ok(BulkString::new(data.to_vec()))
}

fn bulk_string_len(input: &mut &[u8]) -> PResult<()> {
    let len: i64 = integer.parse_next(input)?;
    if len == 0 || len == -1 {
        return Ok(());
    } else if len < 0 {
        return Err(err_cut("bulk string length must be non-negative"));
    }
    terminated(take(len as usize), CRLF)
        .value(())
        .parse_next(input)
}

// "*-1\r\n"
fn null_array(input: &mut &[u8]) -> PResult<RespArray> {
    "-1\r\n".value(RespArray::null()).parse_next(input)
}

// - "*2\r\n$3\r\nget\r\n$5\r\nhello\r\n"
#[allow(clippy::comparison_chain)]
fn array(input: &mut &[u8]) -> PResult<RespArray> {
    let len: i64 = integer.parse_next(input)?;
    if len == 0 {
        return Ok(RespArray::new(vec![]));
    } else if len < 0 {
        return Err(err_cut("array length must be non-negative"));
    }
    let len = len as usize;
    let mut frames = Vec::with_capacity(len);
    for _ in 0..len {
        let frame = parse_frame(input)?;
        frames.push(frame);
    }
    Ok(RespArray::new(frames))
}

fn array_len(input: &mut &[u8]) -> PResult<()> {
    let len: i64 = integer.parse_next(input)?;
    if len == 0 || len == -1 {
        return Ok(());
    } else if len < 0 {
        return Err(err_cut("array length must be non-negative"));
    }
    for _ in 0..len {
        parse_frame_len(input)?;
    }
    Ok(())
}

// Booleans: "#<t|f>\r\n"
fn boolean(input: &mut &[u8]) -> PResult<bool> {
    let v = alt((b't', b'f')).parse_next(input)?;
    Ok(v == b't')
}

// - float: ",3.14\r\n"
fn double(input: &mut &[u8]) -> PResult<f64> {
    terminated(float, CRLF).parse_next(input)
}

// - map: "%2\r\n$3\r\nkey\r\n$5\r\nvalue\r\n$3\r\nkey\r\n$5\r\nvalue\r\n"
fn map(input: &mut &[u8]) -> PResult<RespMap> {
    let len: i64 = integer.parse_next(input)?;
    if len <= 0 {
        return Err(err_cut("map length must be non-negative"));
    }
    let len = (len / 2) as usize;
    let mut frames = BTreeMap::new();
    for _ in 0..len {
        let key = preceded('+', parse_string).parse_next(input)?;
        let value = parse_frame(input)?;
        frames.insert(key, value);
    }
    Ok(RespMap(frames))
}

fn map_len(input: &mut &[u8]) -> PResult<()> {
    let len: i64 = integer.parse_next(input)?;
    if len <= 0 {
        return Err(err_cut("map length must be non-negative"));
    }
    let len = (len / 2) as usize;
    for _ in 0..len {
        terminated(take_until(0.., CRLF), CRLF)
            .value(())
            .parse_next(input)?;
        parse_frame_len(input)?;
    }
    Ok(())
}

fn parse_string(input: &mut &[u8]) -> PResult<String> {
    terminated(take_until(0.., CRLF), CRLF)
        .map(|s: &[u8]| String::from_utf8_lossy(s).into_owned())
        .parse_next(input)
}

fn err_cut(_s: impl Into<String>) -> ErrMode<ContextError> {
    let context = ContextError::default();
    ErrMode::Cut(context)
}
