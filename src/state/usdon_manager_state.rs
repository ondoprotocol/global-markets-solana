use anchor_lang::prelude::*;

/// USDonManagerState state account - tracks global configuration for the USDon stablecoin system
#[account]
#[derive(InitSpace)]
pub struct USDonManagerState {
    // The USDonManager initializer's address
    pub owner: Pubkey,

    // The USDon mint address
    pub usdon_mint: Pubkey,

    // Whether oracle pricing is enabled for USDon operations
    pub oracle_price_enabled: bool,

    // The length of time (in seconds) that an oracle price is considered valid
    pub oracle_price_max_age: u64,

    // The USDC price oracle account used to fetch the USDC price
    pub usdc_price_update: Pubkey,

    // The USDC vault address used for backing USDon
    pub usdc_vault: Pubkey,

    // The USDon vault address used for backing USDon
    pub usdon_vault: Pubkey,

    // The bump used to derive the PDA for this account
    // Stored so we don't need to recalculate it later
    pub bump: u8,
}
