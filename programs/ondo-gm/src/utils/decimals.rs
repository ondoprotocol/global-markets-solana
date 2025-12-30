use anchor_lang::prelude::*;

use crate::errors::OndoError;

/// Normalize an amount from one decimal precision to another
/// # Arguments
/// * `amount` - The amount to normalize
/// * `from_decimals` - The current decimal precision of the amount
/// * `to_decimals` - The target decimal precision to normalize to
/// # Returns
/// * `Result<u64>` - The normalized amount
#[inline(always)]
pub fn normalize_decimals(
    amount: u64,
    from_decimals: u8,
    to_decimals: u8,
    round_up: bool,
) -> Result<u64> {
    if to_decimals > from_decimals {
        amount
            .checked_mul(10u64.pow((to_decimals - from_decimals) as u32))
            .ok_or(OndoError::MathOverflow.into())
    } else if from_decimals > to_decimals {
        let d = 10u128.pow((from_decimals - to_decimals) as u32);

        // ceil(a/b) = (a + b - 1) / b
        let c = if round_up { d - 1 } else { 0 };

        let q = (amount as u128 + c) / d;

        Ok(u64::try_from(q).map_err(|_| OndoError::MathOverflow)?)
    } else {
        Ok(amount)
    }
}
