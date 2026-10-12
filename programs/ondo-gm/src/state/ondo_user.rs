use anchor_lang::prelude::*;

use crate::{errors::OndoError, utils::calculate_capacity_used};

/// OndoUser state account - tracks trading capacity for a user and mint pair
#[account]
#[derive(InitSpace)]
pub struct OndoUser {
    // The address of the user who owns this OndoUser account
    pub owner: Pubkey,

    // The mint associated with this OndoUser account
    pub mint: Pubkey,

    // Maximum USD notional capacity per direction for this user/mint, scaled to 9 decimals
    pub rate_limit: Option<u64>,

    // Limit window defines the time frame (in seconds) for the rate limit
    pub limit_window: Option<u64>,

    // Mint-side USD notional capacity used, scaled to 9 decimals and decayed over time
    pub mint_capacity_used: Option<u64>,

    // The timestamp of the last update to the mint capacity
    pub mint_last_updated: Option<i64>,

    // Redeem-side USD notional capacity used, scaled to 9 decimals and decayed over time
    pub redeem_capacity_used: Option<u64>,

    // The timestamp of the last update to the redeem capacity
    pub redeem_last_updated: Option<i64>,

    // The bump used to derive the PDA for this account
    // Stored so we don't need to recalculate it later
    pub bump: u8,
}

impl OndoUser {
    #[inline(always)]
    /// Initializes the Ondo user account if it is not already initialized.
    /// Sets the owner, mint, rate limit, limit window, and bump values.
    pub fn initialize(
        &mut self,
        user: &Pubkey,
        mint: &Pubkey,
        rate_limit: Option<u64>,
        limit_window: Option<u64>,
        bump: u8,
    ) {
        if self.owner != *user {
            self.owner = *user;
            self.mint = *mint;
            self.rate_limit = rate_limit;
            self.limit_window = limit_window;
            self.mint_capacity_used = Some(0);
            self.mint_last_updated = None;
            self.redeem_capacity_used = Some(0);
            self.redeem_last_updated = None;
            self.bump = bump;
        }
    }

    pub fn check_and_update_limit(
        &mut self,
        amount: u64,
        current_timestamp: i64,
        is_buy: bool,
    ) -> Result<()> {
        if let (Some(user_rate_limit), Some(user_limit_window)) =
            (self.rate_limit, self.limit_window)
        {
            let (user_capacity_used, user_last_updated) = if is_buy {
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
                .checked_sub(user_last_updated)
                .ok_or(OndoError::MathOverflow)?;

            // Calculate current capacity used with linear decay
            let current_user_capacity_used = calculate_capacity_used(
                time_since_last_update,
                user_limit_window,
                user_capacity_used,
                user_rate_limit,
            )?;

            // Calculate available capacity
            let available_user_capacity = if user_rate_limit > current_user_capacity_used {
                user_rate_limit
                    .checked_sub(current_user_capacity_used)
                    .ok_or(OndoError::MathOverflow)?
            } else {
                0
            };

            // Check if the requested amount exceeds available capacity
            if amount > available_user_capacity {
                msg!(
                    "User rate limit exceeded: requested {} > available {}. rate_limit={}, capacity_used={}, window={}, time_since_update={}",
                    amount, available_user_capacity, user_rate_limit, current_user_capacity_used,
                    user_limit_window, time_since_last_update
                );
                return Err(OndoError::InvalidRateLimit.into());
            }

            // Update user rate limit state
            if is_buy {
                self.mint_capacity_used = Some(
                    current_user_capacity_used
                        .checked_add(amount)
                        .ok_or(OndoError::MathOverflow)?,
                );
                self.mint_last_updated = Some(current_timestamp);
            } else {
                self.redeem_capacity_used = Some(
                    current_user_capacity_used
                        .checked_add(amount)
                        .ok_or(OndoError::MathOverflow)?,
                );
                self.redeem_last_updated = Some(current_timestamp);
            }
        } else {
            // User limits are not properly configured - fail the transaction
            return Err(OndoError::InvalidRateLimit.into());
        }

        Ok(())
    }
}
