use anchor_lang::prelude::*;

/// OracleSanityCheck state account - tracks sanity check parameters for a specific mint
#[account]
#[derive(InitSpace)]
pub struct OracleSanityCheck {
    // The GM Token associated with this sanity check
    pub mint: Pubkey,

    // The last known good price for the GM Token
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
