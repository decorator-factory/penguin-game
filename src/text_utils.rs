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

pub const fn __const_total_len(chunks: &[&[u8]]) -> usize {
    let mut len = 0;
    let mut i = 0;
    while i < chunks.len() {
        len += chunks[i].len();
        i += 1;
    }
    len
}

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

macro_rules! concat_bytes {
    // https://github.com/rust-lang/rust/issues/87555#issuecomment-1773972133
	($e:expr) => {{
        const CHUNKS: &[&[u8]] = &$e;
        const N: usize = $crate::text_utils::__const_total_len(CHUNKS);
        const BUF: [u8; N] = $crate::text_utils::__const_concat(CHUNKS);
        &BUF
    }}
}
pub(crate) use concat_bytes;

#[cfg(test)]
mod test {
    #[test]
    fn concat_bytes_works() {
        const TEST: &[u8] = concat_bytes!([b"foo\x00", /* comment */ b"bar", b"baz"]);
        assert_eq!(TEST, b"foo\x00barbaz");
    }
}
