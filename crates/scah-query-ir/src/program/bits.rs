//! Fixed-width bitsets stored as `u64` word slices.
//!
//! Every [`Program`](super::Program) mask and every matcher frame column holds
//! one bit per compiled step, so all of them share the program's word count.
//! The helpers take plain slices so masks can live in one flat allocation.

/// Number of `u64` words needed to hold `bits` bits (at least one).
#[inline]
pub const fn words_for(bits: usize) -> usize {
    if bits == 0 { 1 } else { bits.div_ceil(64) }
}

#[inline]
pub fn set(bits: &mut [u64], index: usize) {
    bits[index / 64] |= 1 << (index % 64);
}

#[inline]
pub fn clear(bits: &mut [u64], index: usize) {
    bits[index / 64] &= !(1 << (index % 64));
}

#[inline]
pub fn contains(bits: &[u64], index: usize) -> bool {
    bits[index / 64] & (1 << (index % 64)) != 0
}

#[inline]
pub fn any(bits: &[u64]) -> bool {
    bits.iter().any(|word| *word != 0)
}

#[inline]
pub fn intersects(left: &[u64], right: &[u64]) -> bool {
    left.iter()
        .zip(right)
        .any(|(left, right)| left & right != 0)
}

#[inline]
pub fn or_assign(target: &mut [u64], source: &[u64]) {
    for (target, source) in target.iter_mut().zip(source) {
        *target |= source;
    }
}

#[inline]
pub fn and_assign(target: &mut [u64], source: &[u64]) {
    for (target, source) in target.iter_mut().zip(source) {
        *target &= source;
    }
}

#[inline]
pub fn and_not_assign(target: &mut [u64], source: &[u64]) {
    for (target, source) in target.iter_mut().zip(source) {
        *target &= !source;
    }
}

/// `target |= (source << 1) & mask`, carrying across words.
///
/// Step `s` follows step `s - 1` inside a selector alternative, so shifting a
/// frame's matched bits left by one yields the steps those matches enable.
/// `mask` drops bits that crossed an alternative boundary.
#[inline]
pub fn or_shifted_and(target: &mut [u64], source: &[u64], mask: &[u64]) {
    let mut carry = 0;
    for ((target, source), mask) in target.iter_mut().zip(source).zip(mask) {
        *target |= ((source << 1) | carry) & mask;
        carry = source >> 63;
    }
}

/// Iterate the indices of set bits in ascending order.
#[inline]
pub fn iter_ones(bits: &[u64]) -> impl Iterator<Item = usize> + '_ {
    bits.iter().enumerate().flat_map(|(word_index, &word)| {
        let mut word = word;
        std::iter::from_fn(move || {
            if word == 0 {
                return None;
            }
            let bit = word.trailing_zeros() as usize;
            word &= word - 1;
            Some(word_index * 64 + bit)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shift_carries_into_the_next_word() {
        let mut target = [0_u64; 2];
        or_shifted_and(&mut target, &[1 << 63, 1], &[u64::MAX, u64::MAX]);
        assert_eq!(target, [0, 0b11]);
    }

    #[test]
    fn shift_respects_the_mask() {
        let mut target = [0_u64; 1];
        or_shifted_and(&mut target, &[0b101], &[0b0010]);
        assert_eq!(target, [0b0010]);
    }

    #[test]
    fn iterates_set_bits_across_words() {
        let mut bits = [0_u64; 3];
        for index in [0, 5, 63, 64, 130] {
            set(&mut bits, index);
        }
        assert_eq!(iter_ones(&bits).collect::<Vec<_>>(), [0, 5, 63, 64, 130]);
        clear(&mut bits, 63);
        assert!(!contains(&bits, 63));
        assert!(contains(&bits, 64));
    }
}
