pub fn uleb128(v: u64) -> Vec<u8> {
    let mut out = Vec::new();
    let mut rest = v;
    loop {
        let mut byte = (rest & 0x7f) as u8;
        rest >>= 7;
        if rest != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if rest == 0 {
            break;
        }
    }
    out
}

pub fn sleb128(v: i64) -> Vec<u8> {
    let mut out = Vec::new();
    let mut rest = v;
    loop {
        let byte = (rest as u8) & 0x7f;
        rest >>= 7;
        let done = (rest == 0 && (byte & 0x40) == 0) || (rest == -1 && (byte & 0x40) != 0);
        out.push(if done { byte } else { byte | 0x80 });
        if done {
            break;
        }
    }
    out
}

pub fn put_u8(out: &mut Vec<u8>, v: u8) {
    out.push(v);
}

pub fn put_uleb(out: &mut Vec<u8>, v: u64) {
    out.extend_from_slice(&uleb128(v));
}

pub fn put_sleb(out: &mut Vec<u8>, v: i64) {
    out.extend_from_slice(&sleb128(v));
}

pub fn put_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    put_uleb(out, bytes.len() as u64);
    out.extend_from_slice(bytes);
}

pub fn section(id: u8, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(2 + payload.len());
    out.push(id);
    put_uleb(&mut out, payload.len() as u64);
    out.extend_from_slice(payload);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode_uleb(bytes: &[u8]) -> u64 {
        let mut result = 0u64;
        let mut shift = 0;
        for b in bytes {
            result |= ((b & 0x7f) as u64) << shift;
            if b & 0x80 == 0 {
                break;
            }
            shift += 7;
        }
        result
    }

    fn decode_sleb(bytes: &[u8]) -> i64 {
        let mut result = 0i64;
        let mut shift = 0;
        let mut byte = 0u8;
        for b in bytes {
            byte = *b;
            result |= ((byte & 0x7f) as i64).wrapping_shl(shift);
            shift += 7;
            if byte & 0x80 == 0 {
                break;
            }
        }
        if shift < 64 && byte & 0x40 != 0 {
            result |= (!0i64).wrapping_shl(shift);
        }
        result
    }

    #[test]
    fn uleb128_roundtrip() {
        for v in [0u64, 1, 127, 128, 624485, 123456789, u64::MAX] {
            let bytes = uleb128(v);
            assert_eq!(decode_uleb(&bytes), v, "bytes {:?}", bytes);
        }
    }

    #[test]
    fn sleb128_roundtrip() {
        for v in [0i64, 1, 127, 128, 624485, -1, -123456, i64::MIN, i64::MAX] {
            let bytes = sleb128(v);
            assert_eq!(decode_sleb(&bytes), v, "bytes {:?}", bytes);
        }
    }

    #[test]
    fn leb128_known_encodings() {
        assert_eq!(uleb128(0), vec![0x00]);
        assert_eq!(uleb128(1), vec![0x01]);
        assert_eq!(uleb128(127), vec![0x7f]);
        assert_eq!(uleb128(128), vec![0x80, 0x01]);
        assert_eq!(uleb128(624485), vec![0xe5, 0x8e, 0x26]);
        assert_eq!(sleb128(0), vec![0x00]);
        assert_eq!(sleb128(1), vec![0x01]);
        assert_eq!(sleb128(-1), vec![0x7f]);
        assert_eq!(sleb128(-123456), vec![0xc0, 0xbb, 0x78]);
    }

    #[test]
    fn section_length_is_uleb_of_payload() {
        let s = section(1, &[0x60, 0x00, 0x00]);
        assert_eq!(s[0], 1);
        assert_eq!(s[1], 3);
        assert_eq!(&s[2..], &[0x60, 0x00, 0x00]);
    }
}
