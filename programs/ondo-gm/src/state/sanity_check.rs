use anchor_lang::prelude::*;

use crate::{constants::BASIS_POINTS_DIVISOR, errors::OndoError, utils::mul_div};

/// OracleSanityCheck state account - tracks sanity check parameters for a specific mint
#[account]
#[derive(InitSpace)]
pub struct OracleSanityCheck {
    // The mint associated with this sanity check (GM or portfolio in trading paths)
    pub mint: Pubkey,

    // The reference USD price per whole token, scaled to 9 decimals
    pub last_price: u64,

    // The allowed deviation in basis points (bps) from the last known good price
    pub allowed_deviation_bps: u64,

    // The maximum time delay (in seconds) for the price to be considered valid
    pub max_time_delay: i64,

    // The timestamp of the last price update
    pub price_last_updated: i64,

    // The bump used to derive the PDA for this account
    // Stored so we don't need to recalculate it later
    pub bump: u8,
}

impl OracleSanityCheck {
    pub fn validate_freshness(&self, current_timestamp: i64) -> Result<()> {
        // Check time since last price update
        let elapsed_time = current_timestamp
            .checked_sub(self.price_last_updated)
            .ok_or(OndoError::MathOverflow)?;

        // Ensure the price data is recent enough
        require_gte!(
            self.max_time_delay,
            elapsed_time,
            OndoError::MaxTimeDelayExceeded
        );

        Ok(())
    }

    pub fn sanity_check(&self, price: u64, current_timestamp: i64) -> Result<()> {
        // Perform sanity checks on the token
        // Ensure the price is within a reasonable range of the last price
        let deviation = mul_div(
            self.last_price,
            self.allowed_deviation_bps,
            BASIS_POINTS_DIVISOR,
            false,
        )?;

        // Calculate maximum acceptable price
        let max_price = self
            .last_price
            .checked_add(deviation)
            .ok_or(OndoError::MathOverflow)?;

        // Calculate minimum acceptable price
        let min_price = self
            .last_price
            .checked_sub(deviation)
            .ok_or(OndoError::MathOverflow)?;

        // Check if the price is within the allowed deviation range
        if price > max_price {
            msg!(
                "Price sanity check failed: price {} exceeds max_price {}. last_price={}, percentage_bp={}, deviation={}",
                price, max_price, self.last_price, self.allowed_deviation_bps, deviation
            );
            return Err(OndoError::PriceExceedsMaxDeviation.into());
        } else if price < min_price {
            msg!(
                "Price sanity check failed: price {} below min_price {}. last_price={}, percentage_bp={}, deviation={}",
                price, min_price, self.last_price, self.allowed_deviation_bps, deviation
            );
            return Err(OndoError::PriceBelowMinDeviation.into());
        }

        self.validate_freshness(current_timestamp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::SECONDS_PER_DAY;

    const PRICE_LAST_UPDATED: i64 = 1_000_000_000;

    fn oracle_sanity_check(last_price: u64, allowed_deviation_bps: u64) -> OracleSanityCheck {
        OracleSanityCheck {
            mint: Pubkey::default(),
            last_price,
            allowed_deviation_bps,
            max_time_delay: 7 * SECONDS_PER_DAY,
            price_last_updated: PRICE_LAST_UPDATED,
            bump: 0,
        }
    }

    #[test]
    fn test_sanity_check_representable_bounds_do_not_overflow() {
        // $2M at 9 decimals with 100% deviation: last_price * allowed_deviation_bps
        // exceeds u64::MAX, but the deviation and both bounds are representable.
        let sanity_check = oracle_sanity_check(2_000_000_000_000_000, 10_000);

        assert!(sanity_check
            .sanity_check(2_000_000_000_000_000, PRICE_LAST_UPDATED)
            .is_ok());
        assert!(sanity_check
            .sanity_check(4_000_000_000_000_000, PRICE_LAST_UPDATED)
            .is_ok());
        assert_eq!(
            sanity_check
                .sanity_check(4_000_000_000_000_001, PRICE_LAST_UPDATED)
                .unwrap_err(),
            OndoError::PriceExceedsMaxDeviation.into()
        );
    }

    #[test]
    fn test_sanity_check_unrepresentable_max_price_is_rejected() {
        // At 100% deviation max_price is 2 * last_price, so u64::MAX / 2 is the
        // largest last_price whose upper bound still fits in a u64.
        let sanity_check = oracle_sanity_check(u64::MAX / 2, 10_000);
        assert!(sanity_check
            .sanity_check(u64::MAX / 2, PRICE_LAST_UPDATED)
            .is_ok());

        let sanity_check = oracle_sanity_check(u64::MAX / 2 + 1, 10_000);
        assert_eq!(
            sanity_check
                .sanity_check(u64::MAX / 2 + 1, PRICE_LAST_UPDATED)
                .unwrap_err(),
            OndoError::MathOverflow.into()
        );
    }
}
