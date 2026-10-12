use anchor_lang::prelude::*;

use crate::{errors::OndoError, utils::calculate_capacity_used};

/// TokenLimit state account - tracks per-mint trading pauses and USD notional limits
#[account]
#[derive(InitSpace)]
pub struct TokenLimit {
    // The mint associated with this TokenLimit account
    pub mint: Pubkey,

    // Maximum USD notional capacity per direction across users, scaled to 9 decimals
    pub rate_limit: Option<u64>,

    // Limit window defines the time frame (in seconds) for the global rate limit
    pub limit_window: Option<u64>,

    // Mint-side USD notional capacity used, scaled to 9 decimals and decayed over time
    pub mint_capacity_used: Option<u64>,

    // The timestamp of the last update to the mint capacity
    pub mint_last_updated: Option<i64>,

    // Redeem-side USD notional capacity used, scaled to 9 decimals and decayed over time
    pub redeem_capacity_used: Option<u64>,

    // The timestamp of the last update to the redeem capacity
    pub redeem_last_updated: Option<i64>,

    // Whether redemptions are paused for this token
    // If true, then redemptions are not allowed
    pub redemption_paused: bool,

    // Whether minting is paused for this token
    // If true, then minting is not allowed
    pub minting_paused: bool,

    // Default per-user USD notional capacity per direction, scaled to 9 decimals
    pub default_user_rate_limit: Option<u64>,

    // Default user capacity decay window for this token, in seconds
    pub default_user_limit_window: Option<u64>,

    // The bump used to derive the PDA for this account
    // Stored so we don't need to recalculate it later
    pub bump: u8,
}

impl TokenLimit {
    /// Checks and updates the token-level rate limit state.
    /// # Arguments
    /// * `amount` - Rounded-up USD notional of the trade, scaled to 9 decimals.
    /// * `current_timestamp` - The current timestamp.
    /// * `is_buy` - A boolean indicating if the transaction is a buy (true) or sell (false).
    /// # Returns
    /// * `Result<()>` - Ok if the check passes, Err otherwise.
    #[inline(always)]
    pub fn check_and_update_limit(
        &mut self,
        amount: u64,
        current_timestamp: i64,
        is_buy: bool,
    ) -> Result<()> {
        // Check if token-level rate limits are configured
        if let (Some(token_rate_limit), Some(token_limit_window)) =
            (self.rate_limit, self.limit_window)
        {
            let (token_capacity_used, token_last_updated) = if is_buy {
                (
                    self.mint_capacity_used.ok_or(OndoError::DataMismatch)?,
                    self.mint_last_updated.unwrap_or(current_timestamp),
                )
            } else {
                (
                    self.redeem_capacity_used.ok_or(OndoError::DataMismatch)?,
                    self.redeem_last_updated.unwrap_or(current_timestamp),
                )
            };

            // Calculate time since last update
            let time_since_last_update = current_timestamp
                .checked_sub(token_last_updated)
                .ok_or(OndoError::MathOverflow)?;

            // Calculate current capacity used with linear decay
            let current_token_capacity_used = calculate_capacity_used(
                time_since_last_update,
                token_limit_window,
                token_capacity_used,
                token_rate_limit,
            )?;

            // Calculate available capacity
            let available_token_capacity = if token_rate_limit > current_token_capacity_used {
                token_rate_limit
                    .checked_sub(current_token_capacity_used)
                    .ok_or(OndoError::MathOverflow)?
            } else {
                0
            };

            // Check if the requested amount exceeds available capacity
            if amount > available_token_capacity {
                msg!(
                    "Token rate limit exceeded: requested {} > available {}. rate_limit={}, capacity_used={}, window={}, time_since_update={}",
                    amount, available_token_capacity, token_rate_limit, current_token_capacity_used,
                    token_limit_window, time_since_last_update
                );
                return Err(OndoError::InvalidRateLimit.into());
            }

            // Update token rate limit state
            if is_buy {
                self.mint_capacity_used = Some(
                    current_token_capacity_used
                        .checked_add(amount)
                        .ok_or(OndoError::MathOverflow)?,
                );
                self.mint_last_updated = Some(current_timestamp);
            } else {
                self.redeem_capacity_used = Some(
                    current_token_capacity_used
                        .checked_add(amount)
                        .ok_or(OndoError::MathOverflow)?,
                );
                self.redeem_last_updated = Some(current_timestamp);
            }
        } else {
            // Token limits are not properly configured - fail the transaction
            return Err(OndoError::InvalidRateLimit.into());
        }

        Ok(())
    }
}
