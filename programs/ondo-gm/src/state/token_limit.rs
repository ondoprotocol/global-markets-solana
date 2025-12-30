use anchor_lang::prelude::*;

/// TokenLimit state account - tracks global token limit parameters for a specific GM Token
#[account]
#[derive(InitSpace)]
pub struct TokenLimit {
    // The GM Token mint associated with this TokenLimit account
    pub mint: Pubkey,

    // Rate limit defines the maximum amount of tokens that can be minted/redeemed globally
    pub rate_limit: Option<u64>,

    // Limit window defines the time frame (in seconds) for the global rate limit
    pub limit_window: Option<u64>,

    // The amount of mint capacity used in the current limit window
    pub mint_capacity_used: Option<u64>,

    // The timestamp of the last update to the mint capacity
    pub mint_last_updated: Option<i64>,

    // The amount of redeem capacity used in the current limit window
    pub redeem_capacity_used: Option<u64>,

    // The timestamp of the last update to the redeem capacity
    pub redeem_last_updated: Option<i64>,

    // Whether redemptions are paused for this token
    // If true, then redemptions are not allowed
    pub redemption_paused: bool,

    // Whether minting is paused for this token
    // If true, then minting is not allowed
    pub minting_paused: bool,

    // Default user rate limit for this token
    pub default_user_rate_limit: Option<u64>,

    // Default user limit window for this token
    pub default_user_limit_window: Option<u64>,

    // The bump used to derive the PDA for this account
    // Stored so we don't need to recalculate it later
    pub bump: u8,
}
