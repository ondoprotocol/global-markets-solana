use anchor_lang::prelude::*;

use crate::errors::OndoError;

/// GM Token Manager State account - tracks global state for GM Token operations
#[account]
#[derive(InitSpace)]
pub struct GMTokenManagerState {
    // Monotonically increasing execution ID for mint/redeem operations
    pub execution_id: Option<u128>,

    // True if the GM factory is paused - prevents new GM and USDon mints from being created
    pub factory_paused: bool,

    // True if redemption is paused
    pub redemption_paused: bool,

    // True if minting is paused
    pub minting_paused: bool,

    // Bump used to derive the PDA for this account
    // Stored so we don't need to recalculate it later
    pub bump: u8,

    /// Ethereum address (20 bytes) for secp256k1 signature verification
    /// Used to verify attestation signatures for buy/sell operations
    /// All zeros ([0u8; 20]) means not set
    pub attestation_signer_secp: [u8; 20],

    /// Legacy trading-hours offset from UTC in seconds, retained for account compatibility.
    /// Set at initialization; no trading-hours check uses this field.
    pub trading_hours_offset: i64,
}

impl GMTokenManagerState {
    pub fn next_execution_id(&mut self) -> Result<u128> {
        let current_id = self.execution_id.unwrap_or(0);
        let next_id = current_id.checked_add(1).ok_or(OndoError::MathOverflow)?;
        self.execution_id = Some(next_id);
        Ok(next_id)
    }
}
