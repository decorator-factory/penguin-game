/// Split a slice into two parts. All the elements in the "prefix" part
/// match the predicate, and the first elemenet of the "rest" part doesn't
/// match the predicate.
use std::rc::Rc;

pub(crate) fn split_while<T, F>(slice: &[T], mut pred: F) -> (&[T], &[T])
where
    F: FnMut(&T) -> bool,
{
    if slice.is_empty() {
        return (slice, slice);
    }

    match slice.iter().position(|x| !pred(x)) {
        Some(index) => (&slice[..index], &slice[index..]),
        None => (slice, &slice[slice.len()..]),
    }
}

#[doc(hidden)]
pub const fn __const_total_len(chunks: &[&[u8]]) -> usize {
    let mut len = 0;
    let mut i = 0;
    while i < chunks.len() {
        len += chunks[i].len();
        i += 1;
    }
    len
}

#[doc(hidden)]
pub const fn __const_concat<const N: usize>(chunks: &[&[u8]]) -> [u8; N] {
    let mut buf = [0u8; N];
    let mut i = 0;
    let mut buf_idx = 0;
    while i < chunks.len() {
        let len = chunks[i].len();

        // buf.get_mut is not stable in const contexts :(
        let mut j = 0;
        while j < len {
            buf[buf_idx + j] = chunks[i][j];
            j += 1;
        }

        buf_idx += len;
        i += 1;
    }
    buf
}

macro_rules! cat {
    // This is similar to unstable concat_bytes!
    // Adapted and simplified from: https://github.com/rust-lang/rust/issues/87555#issuecomment-1773972133
	($($e:expr),* $(,)?) => {{
        const CHUNKS: &[&[u8]] = &[$(
            $e,
        )*];
        const N: usize = $crate::text_utils::__const_total_len(CHUNKS);
        const BUF: [u8; N] = $crate::text_utils::__const_concat(CHUNKS);
        &BUF
    }}
}
pub(crate) use cat;

pub(crate) fn split_from_bytes<T: bytemuck::AnyBitPattern>(bytes: &[u8]) -> (&T, &[u8]) {
    let size = core::mem::size_of::<T>();
    assert!(bytes.len() >= size, "Received bytes, {} but need to split off {}", bytes.len(), size);
    (bytemuck::from_bytes(&bytes[..size]), &bytes[size..])
}

pub(crate) fn rc_str_from_utf8(bytes: Rc<[u8]>) -> Result<Rc<str>, core::str::Utf8Error> {
    str::from_utf8(bytes.as_ref())?;

    // SAFETY: checked that bytes are valid UTF-8
    //         str and [u8] have the same alignment and are both DST
    Ok(unsafe { Rc::from_raw(Rc::into_raw(bytes) as *const str) })
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn concat_bytes_works() {
        const TEST: &[u8] = cat!(b"foo\x00", /* comment */ b"bar", b"baz");
        assert_eq!(TEST, b"foo\x00barbaz");
    }

    #[test]
    pub fn split_while_empty() {
        let (left, right) = split_while(b"aaaabcd", |c| *c == b'?');
        assert_eq!((left, right), (&b""[..], &b"aaaabcd"[..]));
    }

    #[test]
    pub fn split_while_empty_string() {
        let (left, right) = split_while(b"", |c| *c == b'?');
        assert_eq!((left, right), (&b""[..], &b""[..]));
    }

    #[test]
    pub fn split_while_mixed() {
        let (left, right) = split_while(b"aaaabcd", |c| *c == b'a');
        assert_eq!((left, right), (&b"aaaa"[..], &b"bcd"[..]));
    }

    #[test]
    pub fn rc_from_utf8_works() {
        let bytes: Rc<[u8]> = (&b"banana"[..]).into();
        let _copy1 = Rc::clone(&bytes);
        let _copy2 = Rc::clone(&bytes);
        assert_eq!(rc_str_from_utf8(bytes), Ok("banana".into()));
    }

    #[test]
    pub fn rc_from_utf8_fail() {
        let bytes: Rc<[u8]> = (&b"\xd8\x00"[..]).into();
        let _copy1 = Rc::clone(&bytes);
        let _copy2 = Rc::clone(&bytes);
        assert!(rc_str_from_utf8(bytes).is_err());
    }
}
