//! Byte encodings shared by the CFF structures: DICT operands (TN #5176 §4, Table 3),
//! Type 2 charstring numbers (TN #5177 §3.2) and INDEX (TN #5176 §5).

use crate::CffError;

/// Appends a DICT integer in its shortest form: 1 byte (−107…107), 2 bytes
/// (±108…±1131), 3 bytes (prefix 28, 16-bit) or 5 bytes (prefix 29, 32-bit).
pub(crate) fn dict_int(out: &mut Vec<u8>, value: i32) {
    if !short_int(out, value) {
        match i16::try_from(value) {
            Ok(v) => {
                out.push(28);
                out.extend(v.to_be_bytes());
            }
            Err(_) => dict_int32(out, value),
        }
    }
}

/// Appends a DICT integer in the fixed 5-byte form (prefix 29). Offsets use it so the
/// Top DICT has the same length whatever the offsets are.
pub(crate) fn dict_int32(out: &mut Vec<u8>, value: i32) {
    out.push(29);
    out.extend(value.to_be_bytes());
}

/// Appends a DICT real (prefix 30): the decimal digits as nibbles, `a` for the point,
/// `e` for the minus sign, `f` to end, padded with `f` to a whole byte.
///
/// `value` must be finite: the only caller passes `1 / unitsPerEm`.
pub(crate) fn dict_real(out: &mut Vec<u8>, value: f64) {
    // Display gives the shortest decimal that reads back to the same f64, and never
    // uses an exponent. "0.5" is written ".5", as in TN #5176.
    let text = format!("{value}");
    let text = text
        .strip_prefix("0.")
        .map(|rest| format!(".{rest}"))
        .or_else(|| text.strip_prefix("-0.").map(|rest| format!("-.{rest}")))
        .unwrap_or(text);
    let mut nibbles: Vec<u8> = text
        .bytes()
        .filter_map(|b| match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'.' => Some(0xa),
            b'-' => Some(0xe),
            _ => None, // not produced by Display for a finite f64
        })
        .collect();
    nibbles.push(0xf);
    if nibbles.len() % 2 == 1 {
        nibbles.push(0xf);
    }
    out.push(30);
    out.extend(nibbles.chunks(2).map(|pair| (pair[0] << 4) | pair[1]));
}

/// Appends a Type 2 charstring integer: 1 or 2 bytes like a DICT integer, or 3 bytes
/// (prefix 28) for the rest of the 16-bit range. Charstrings have no 32-bit integer.
pub(crate) fn charstring_int(out: &mut Vec<u8>, value: i64) -> Result<(), i64> {
    let value = i16::try_from(value).map_err(|_| value)?;
    if !short_int(out, i32::from(value)) {
        out.push(28);
        out.extend(value.to_be_bytes());
    }
    Ok(())
}

/// The 1- and 2-byte integer forms, common to DICTs and charstrings. Returns `false`
/// (and appends nothing) when `value` needs a longer form.
fn short_int(out: &mut Vec<u8>, value: i32) -> bool {
    // The casts are in range: each arm bounds `value` first.
    match value {
        -107..=107 => out.push((value + 139) as u8),
        108..=1131 => {
            let v = value - 108;
            out.extend([(v / 256 + 247) as u8, (v % 256) as u8]);
        }
        -1131..=-108 => {
            let v = -value - 108;
            out.extend([(v / 256 + 251) as u8, (v % 256) as u8]);
        }
        _ => return false,
    }
    true
}

/// Appends an INDEX: a Card16 count, then (if not empty) the offset size, `count + 1`
/// offsets starting at 1 with the smallest size that holds the last one, and the data.
pub(crate) fn index(out: &mut Vec<u8>, items: &[Vec<u8>]) -> Result<(), CffError> {
    let count = u16::try_from(items.len()).map_err(|_| CffError::IndexTooLarge)?;
    out.extend(count.to_be_bytes());
    if count == 0 {
        return Ok(());
    }
    let data_len: usize = items.iter().map(Vec::len).sum();
    let last = u32::try_from(data_len + 1).map_err(|_| CffError::IndexTooLarge)?;
    let off_size: usize = match last {
        0..=0xff => 1,
        0x100..=0xffff => 2,
        0x1_0000..=0xff_ffff => 3,
        _ => 4,
    };
    out.push(off_size as u8);
    let mut offset = 1_u32;
    let push_offset = |out: &mut Vec<u8>, offset: u32| {
        out.extend(&offset.to_be_bytes()[4 - off_size..]);
    };
    push_offset(out, offset);
    for item in items {
        // Cannot overflow: the sum of all lengths plus one fits in u32 (checked above).
        offset += item.len() as u32;
        push_offset(out, offset);
    }
    for item in items {
        out.extend(item);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dict(value: i32) -> Vec<u8> {
        let mut out = Vec::new();
        dict_int(&mut out, value);
        out
    }

    fn charstring(value: i64) -> Vec<u8> {
        let mut out = Vec::new();
        charstring_int(&mut out, value).unwrap();
        out
    }

    fn real(value: f64) -> Vec<u8> {
        let mut out = Vec::new();
        dict_real(&mut out, value);
        out
    }

    #[test]
    fn dict_integers_use_the_shortest_form_at_every_boundary() {
        assert_eq!(dict(-32769), [29, 0xff, 0xff, 0x7f, 0xff]);
        assert_eq!(dict(-32768), [28, 0x80, 0x00]);
        assert_eq!(dict(-1132), [28, 0xfb, 0x94]);
        assert_eq!(dict(-1131), [254, 0xff]);
        assert_eq!(dict(-108), [251, 0x00]);
        assert_eq!(dict(-107), [32]);
        assert_eq!(dict(0), [139]);
        assert_eq!(dict(107), [246]);
        assert_eq!(dict(108), [247, 0x00]);
        assert_eq!(dict(1131), [250, 0xff]);
        assert_eq!(dict(1132), [28, 0x04, 0x6c]);
        assert_eq!(dict(32767), [28, 0x7f, 0xff]);
        assert_eq!(dict(32768), [29, 0x00, 0x00, 0x80, 0x00]);
    }

    #[test]
    fn the_fixed_width_form_is_always_five_bytes() {
        let mut out = Vec::new();
        dict_int32(&mut out, 0);
        dict_int32(&mut out, 100_000);
        assert_eq!(out, [29, 0, 0, 0, 0, 29, 0x00, 0x01, 0x86, 0xa0]);
    }

    #[test]
    fn dict_reals_are_nibble_encoded() {
        // TN #5176 §4: "-2.25" is 1e e2 a2 5f.
        assert_eq!(real(-2.25), [30, 0xe2, 0xa2, 0x5f]);
        assert_eq!(real(0.001), [30, 0xa0, 0x01, 0xff]);
        assert_eq!(real(-0.5), [30, 0xea, 0x5f]);
        assert_eq!(real(1.0), [30, 0x1f]);
        // 1/2048, the FontMatrix scale of a 2048 units-per-em font.
        assert_eq!(
            real(1.0 / 2048.0),
            [30, 0xa0, 0x00, 0x48, 0x82, 0x81, 0x25, 0xff]
        );
    }

    #[test]
    fn charstring_integers_use_the_shortest_form_at_every_boundary() {
        assert_eq!(charstring(-32768), [28, 0x80, 0x00]);
        assert_eq!(charstring(-1132), [28, 0xfb, 0x94]);
        assert_eq!(charstring(-1131), [254, 0xff]);
        assert_eq!(charstring(-108), [251, 0x00]);
        assert_eq!(charstring(-107), [32]);
        assert_eq!(charstring(107), [246]);
        assert_eq!(charstring(108), [247, 0x00]);
        assert_eq!(charstring(1131), [250, 0xff]);
        assert_eq!(charstring(1132), [28, 0x04, 0x6c]);
        assert_eq!(charstring(32767), [28, 0x7f, 0xff]);
    }

    #[test]
    fn charstring_integers_outside_16_bits_are_rejected() {
        let mut out = Vec::new();
        assert_eq!(charstring_int(&mut out, 32768), Err(32768));
        assert_eq!(charstring_int(&mut out, -32769), Err(-32769));
        assert!(out.is_empty());
    }

    fn index_of(items: &[Vec<u8>]) -> Vec<u8> {
        let mut out = Vec::new();
        index(&mut out, items).unwrap();
        out
    }

    #[test]
    fn an_empty_index_is_only_its_count() {
        assert_eq!(index_of(&[]), [0, 0]);
    }

    #[test]
    fn a_small_index_uses_one_byte_offsets() {
        let items = [b"ab".to_vec(), Vec::new(), b"c".to_vec()];
        assert_eq!(index_of(&items), [0, 3, 1, 1, 3, 3, 4, b'a', b'b', b'c']);
    }

    #[test]
    fn an_index_whose_last_offset_needs_two_bytes_uses_two_byte_offsets() {
        let out = index_of(&[vec![7; 255]]);
        assert_eq!(out[..7], [0, 1, 2, 0x00, 0x01, 0x01, 0x00]);
        assert_eq!(out.len(), 7 + 255);
    }

    #[test]
    fn an_index_whose_last_offset_needs_three_bytes_uses_three_byte_offsets() {
        let out = index_of(&[vec![7; 0xffff]]);
        assert_eq!(out[..9], [0, 1, 3, 0, 0, 1, 0x01, 0x00, 0x00]);
        assert_eq!(out.len(), 9 + 0xffff);
    }

    #[test]
    fn an_index_with_more_than_65535_items_is_rejected() {
        let items = vec![Vec::new(); 65536];
        assert_eq!(index(&mut Vec::new(), &items), Err(CffError::IndexTooLarge));
    }
}
