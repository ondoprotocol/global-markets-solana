use anchor_lang::prelude::Result;

use crate::errors::OndoError;

/// Safely computes (n0 * n1) / d with overflow protection
/// Returns error if d is 0 or result overflows u64
/// # Arguments
/// * `n0` - The first multiplicand
/// * `n1` - The second multiplicand
/// * `d` - The divisor
/// * `round_up` - Whether to round up the result if there's a remainder
/// # Returns
/// * `Result<u64>` - The result of (n0 * n1) / d
#[inline(always)]
pub fn mul_div(n0: u64, n1: u64, d: u64, round_up: bool) -> Result<u64> {
    if d == 0 {
        return Err(OndoError::DivideByZero.into());
    }

    let p = (n0 as u128)
        .checked_mul(n1 as u128)
        .ok_or(OndoError::MathOverflow)?;

    let d_u128 = d as u128;

    // ceil(a/b) = (a + b - 1) / b
    let c = if round_up { d_u128 - 1 } else { 0 };

    let result = p.checked_add(c).ok_or(OndoError::MathOverflow)? / d_u128;

    Ok(u64::try_from(result).map_err(|_| OndoError::MathOverflow)?)
}
