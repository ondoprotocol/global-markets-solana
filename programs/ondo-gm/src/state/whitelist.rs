use anchor_lang::prelude::*;

/// Whitelist account - tracks whitelisted addresses.
///
/// Used as a marker account - presence of the account indicates whitelisting.
#[account]
#[derive(InitSpace)]
pub struct Whitelist {
    // The whitelisted user
    pub user: Pubkey,
}
