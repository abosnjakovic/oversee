//! The numeric conversions `std` cannot express through `From` or `TryFrom`.
//!
//! Terminal layout turns integer cell counts into float fractions and back, and
//! the memory panel turns byte counts into gigabytes. There is no lossless
//! conversion for `usize -> f32`, `u64 -> f64` or for any float to an integer,
//! so `as` is unavoidable for those four. Keeping them here, in named functions
//! that state what they do about precision and range, means the rest of the
//! codebase converts through `From`/`TryFrom` and nothing else.
//!
//! Float-to-integer `as` saturates rather than wrapping (Rust 1.45 onwards) and
//! maps NaN to zero, so these cannot produce a nonsense value from a bad input.

#![allow(
    clippy::as_conversions,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

/// Widen a count for float maths.
///
/// Exact up to 2^24 (16.7 million); beyond that the result rounds. Every count
/// widened here is a cell, core or process figure far below that.
pub const fn count_to_f32(n: usize) -> f32 {
    n as f32
}

/// Widen a byte count for float maths.
///
/// Exact up to 2^53 bytes (8 petabytes), which no machine this runs on reports.
pub const fn bytes_to_f64(n: u64) -> f64 {
    n as f64
}

/// Narrow a percentage or fraction to `f32`, the precision the charts store.
pub const fn to_f32(v: f64) -> f32 {
    v as f32
}

/// Round a float down to a count, clamping negatives and NaN to 0 and anything
/// past `usize::MAX` to `usize::MAX`. Never wraps, never panics.
pub const fn to_count(v: f32) -> usize {
    v as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The saturating behaviour is the whole reason these are safe to call on
    /// values derived from live measurements.
    #[test]
    fn to_count_clamps_instead_of_wrapping() {
        assert_eq!(to_count(-1.0), 0, "a negative must not wrap to usize::MAX");
        assert_eq!(to_count(f32::NAN), 0);
        assert_eq!(to_count(f32::INFINITY), usize::MAX);
        assert_eq!(to_count(3.9), 3);
    }
}
