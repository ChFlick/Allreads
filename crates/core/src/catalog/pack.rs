//! Packing a book's word counts into a few bytes per word.
//!
//! A book's counts are pairs of (word id, count), sorted by word id. Each pair
//! is stored as two varints: the gap since the previous word id, then the
//! count. A varint (as in Protocol Buffers) stores 7 bits per byte, with the
//! high bit set on every byte but the last, so numbers below 128 take one
//! byte. Word ids are numbered by how common a word is across the catalog,
//! so the common words a book is full of have small ids and small gaps.

use super::CatalogError;

/// Packs `counts`, which must be sorted by word id without repeats (ids below
/// `u32::MAX`; a catalog has far fewer words).
pub(crate) fn pack(counts: &[(u32, u32)]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(counts.len() * 2);
    let mut next_id = 0;
    for &(id, count) in counts {
        debug_assert!(id >= next_id, "word ids must be sorted and unique");
        write_varint(&mut bytes, id - next_id);
        write_varint(&mut bytes, count);
        next_id = id + 1;
    }
    bytes
}

/// Unpacks what [`pack`] wrote.
///
/// # Errors
/// Returns `Format` if the bytes end in the middle of a pair or a number is
/// too large, which only a damaged file would do.
pub(crate) fn unpack(bytes: &[u8]) -> Result<Vec<(u32, u32)>, CatalogError> {
    let damaged = || CatalogError::Format("damaged word counts".into());
    let mut counts = Vec::new();
    let mut position = 0;
    let mut next_id: u32 = 0;
    while position < bytes.len() {
        let gap = read_varint(bytes, &mut position).ok_or_else(damaged)?;
        let count = read_varint(bytes, &mut position).ok_or_else(damaged)?;
        let id = next_id.checked_add(gap).ok_or_else(damaged)?;
        counts.push((id, count));
        next_id = id.checked_add(1).ok_or_else(damaged)?;
    }
    Ok(counts)
}

fn write_varint(bytes: &mut Vec<u8>, mut value: u32) {
    while value >= 0x80 {
        // The low 7 bits, with the high bit saying "more bytes follow".
        bytes.push((value & 0x7f) as u8 | 0x80);
        value >>= 7;
    }
    bytes.push(value as u8);
}

/// Reads one varint at `position` and moves past it; `None` if the bytes end
/// first or the number does not fit in 32 bits.
fn read_varint(bytes: &[u8], position: &mut usize) -> Option<u32> {
    let mut value: u32 = 0;
    let mut shift = 0;
    loop {
        let byte = *bytes.get(*position)?;
        *position += 1;
        let bits = u32::from(byte & 0x7f);
        if shift == 28 && bits > 0x0f {
            return None;
        }
        value |= bits << shift;
        if byte & 0x80 == 0 {
            return Some(value);
        }
        shift += 7;
        if shift > 28 {
            return None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_survive_packing() {
        let counts = vec![
            (0, 1),
            (1, 300),
            (5, 2),
            (127, 1),
            (128, 70_000),
            (u32::MAX - 1, u32::MAX),
        ];
        assert_eq!(unpack(&pack(&counts)).unwrap(), counts);
        assert!(unpack(&pack(&[])).unwrap().is_empty());
    }

    #[test]
    fn small_ids_and_counts_take_two_bytes_a_word() {
        let counts: Vec<(u32, u32)> = (0..100).map(|id| (id, 3)).collect();
        assert_eq!(pack(&counts).len(), 200);
        // 300 needs a second byte: 0b1_0010_1100 → 0xAC 0x02.
        assert_eq!(pack(&[(0, 300)]), [0x00, 0xac, 0x02]);
    }

    #[test]
    fn damaged_bytes_are_an_error() {
        // A gap with no count after it.
        assert!(unpack(&[0x01]).is_err());
        // A varint that never ends.
        assert!(unpack(&[0x80, 0x80]).is_err());
        // More than 32 bits.
        assert!(unpack(&[0xff, 0xff, 0xff, 0xff, 0x7f, 0x01]).is_err());
    }
}
