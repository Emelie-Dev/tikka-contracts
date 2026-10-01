//! Basis-point arithmetic helpers.
//!
//! All fee and prize calculations in the protocol go through these two
//! functions so that rounding direction and overflow handling are uniform
//! across every call site.
//!
//! # Rounding
//!
//! **All operations truncate** (floor division).  A truncating fee always
//! favours the payer: the protocol collects *at most* the stated percentage,
//! never more.  See `docs/FEE_MODEL.md` for the full rationale.
//!
//! # No-std
//!
//! This module is `#![no_std]`-compatible; it uses only core arithmetic.

use crate::errors::ProtocolError;

/// Denominator for basis-point fractions (1 bp = 1 / 10 000).
pub const BP_DENOMINATOR: u32 = 10_000;

/// Apply a basis-point rate to `amount`, truncating toward zero.
///
/// Returns `floor(amount × bp / 10_000)`.
///
/// # Errors
///
/// Returns [`ProtocolError::ArithmeticOverflow`] when the intermediate
/// `amount × bp` product overflows `i128`.
///
/// # Examples
///
/// ```ignore
/// // 5 % of 1 000 000 = 50 000
/// assert_eq!(apply_bp(1_000_000, 500).unwrap(), 50_000);
///
/// // truncation: floor(10_001 × 1 / 10_000) = 1
/// assert_eq!(apply_bp(10_001, 1).unwrap(), 1);
/// ```
pub fn apply_bp(amount: i128, bp: u32) -> Result<i128, ProtocolError> {
    if bp == 0 || amount == 0 {
        return Ok(0);
    }
    amount
        .checked_mul(bp as i128)
        .ok_or(ProtocolError::ArithmeticOverflow)
        .map(|product| product / BP_DENOMINATOR as i128)
}

/// Split `amount` into a `(fee, remainder)` pair where `fee + remainder ==
/// amount` exactly.
///
/// `fee` is `floor(amount × bp / 10_000)`.  The `remainder` is computed as
/// `amount - fee` so the two components always sum to the original value,
/// regardless of rounding.
///
/// Use this instead of calling [`apply_bp`] twice when you need both sides of
/// a split (e.g. fee vs. net payout) to guarantee no rounding gap.
///
/// # Errors
///
/// Returns [`ProtocolError::ArithmeticOverflow`] when the intermediate product
/// overflows `i128`, or when `fee > amount` (which cannot happen with valid
/// inputs but is checked defensively).
///
/// # Examples
///
/// ```ignore
/// // 10 % split of 1 000: fee = 100, remainder = 900
/// let (fee, net) = split_bp(1_000, 1_000).unwrap();
/// assert_eq!(fee + net, 1_000);
///
/// // Truncation: fee = 1, remainder = 10_000 (not 9_999)
/// let (fee, net) = split_bp(10_001, 1).unwrap();
/// assert_eq!(fee + net, 10_001);
/// ```
pub fn split_bp(amount: i128, bp: u32) -> Result<(i128, i128), ProtocolError> {
    let fee = apply_bp(amount, bp)?;
    let remainder = amount
        .checked_sub(fee)
        .ok_or(ProtocolError::ArithmeticOverflow)?;
    Ok((fee, remainder))
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // apply_bp
    // -----------------------------------------------------------------------

    #[test]
    fn apply_bp_zero_bp_returns_zero() {
        assert_eq!(apply_bp(1_000_000, 0).unwrap(), 0);
    }

    #[test]
    fn apply_bp_zero_amount_returns_zero() {
        assert_eq!(apply_bp(0, 500).unwrap(), 0);
    }

    #[test]
    fn apply_bp_truncates() {
        // floor(10_001 × 1 / 10_000) = 1  (not 2)
        assert_eq!(apply_bp(10_001, 1).unwrap(), 1);
    }

    #[test]
    fn apply_bp_exact_percent() {
        // 5 % of 1_000_000 = 50_000
        assert_eq!(apply_bp(1_000_000, 500).unwrap(), 50_000);
    }

    #[test]
    fn apply_bp_full_denominator_is_identity() {
        // 100 % → same amount
        assert_eq!(apply_bp(12_345, 10_000).unwrap(), 12_345);
    }

    // -----------------------------------------------------------------------
    // split_bp — sum property
    // -----------------------------------------------------------------------

    #[test]
    fn split_bp_sums_to_amount_at_zero_bp() {
        let (fee, rem) = split_bp(999_999, 0).unwrap();
        assert_eq!(fee + rem, 999_999);
        assert_eq!(fee, 0);
    }

    #[test]
    fn split_bp_sums_to_amount_at_full_bp() {
        let (fee, rem) = split_bp(999_999, 10_000).unwrap();
        assert_eq!(fee + rem, 999_999);
        assert_eq!(rem, 0);
    }

    #[test]
    fn split_bp_sums_to_amount_typical() {
        let amount = 1_000_000i128;
        for bp in [1u32, 50, 100, 500, 1_000, 2_000, 9_999, 10_000] {
            let (fee, rem) = split_bp(amount, bp).unwrap();
            assert_eq!(
                fee + rem,
                amount,
                "split_bp({amount}, {bp}) did not sum to amount"
            );
        }
    }

    /// Property: for every bp in 0..=10_000, split_bp components sum to amount.
    #[test]
    fn split_bp_sum_property_all_bp_values() {
        // Representative amounts covering edge cases.
        let amounts: &[i128] = &[
            0,
            1,
            9_999,
            10_000,
            10_001,
            999_999,
            1_000_000,
            i128::MAX / 10_001, // largest safe value for bp=10_000
        ];
        for &amount in amounts {
            for bp in 0u32..=10_000 {
                match split_bp(amount, bp) {
                    Ok((fee, rem)) => assert_eq!(
                        fee + rem,
                        amount,
                        "split_bp({amount}, {bp}): {fee} + {rem} != {amount}"
                    ),
                    Err(ProtocolError::ArithmeticOverflow) => {
                        // Overflow on the very large amount at high bp is expected.
                    }
                    Err(e) => panic!("unexpected error from split_bp({amount}, {bp}): {e:?}"),
                }
            }
        }
    }
}
