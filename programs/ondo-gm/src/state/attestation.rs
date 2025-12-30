use anchor_lang::prelude::*;

/// Attestation account to track consumed attestations
/// each consumed attestation is stored in its own account
#[account]
#[derive(InitSpace)]
pub struct Attestation {
    // The unique identifier of the attestation
    pub attestation_id: [u8; 16],

    // The user who consumed the attestation
    pub creator: Pubkey,

    // The timestamp when the attestation was consumed
    pub created_at: i64,

    // The bump used to derive the PDA for this account
    // Stored so we don't need to recalculate it later
    pub bump: u8,
}
