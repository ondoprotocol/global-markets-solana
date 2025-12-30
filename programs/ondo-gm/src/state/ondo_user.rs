use anchor_lang::prelude::*;

/// OndoUser state account - tracks user-specific data for a GM Token
#[account]
#[derive(InitSpace)]
pub struct OndoUser {
    // The address of the user who owns this OndoUser account
    pub owner: Pubkey,

    // The GM Token mint associated with this OndoUser account
    pub mint: Pubkey,

    // Rate limit defines the maximum amount of tokens that can be minted/redeemed
    pub rate_limit: Option<u64>,

    // Limit window defines the time frame (in seconds) for the rate limit
    pub limit_window: Option<u64>,

    // The amount of mint capacity used in the current limit window
    pub mint_capacity_used: Option<u64>,

    // The timestamp of the last update to the mint capacity
    pub mint_last_updated: Option<i64>,

    // The amount of redeem capacity used in the current limit window
    pub redeem_capacity_used: Option<u64>,

    // The timestamp of the last update to the redeem capacity
    pub redeem_last_updated: Option<i64>,

    // The bump used to derive the PDA for this account
    // Stored so we don't need to recalculate it later
    pub bump: u8,
}
