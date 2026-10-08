use anchor_lang::{
    prelude::*,
    solana_program::{instruction::Instruction, sysvar::instructions},
};
use solana_keccak_hasher::hash;
use solana_sdk_ids::secp256k1_program;

use crate::errors::OndoError;

/// Calculates the keccak256 hash of the quote parameters.
/// # Arguments
/// * `chain_id` - A 32-byte identifier for the blockchain.
/// * `attestation_id` - A unique 16-byte identifier for the attestation.
/// * `side` - A byte indicating the side of the trade (e.g., buy/sell).
/// * `user` - The public key of the user.
/// * `asset` - The public key of the asset (token mint).
/// * `price` - The price associated with the attestation.
/// * `amount` - The amount associated with the attestation.
/// * `expiration` - The expiration timestamp of the attestation.
/// # Returns
/// * `[u8; 32]` - The keccak256 hash of the quote.
#[allow(clippy::too_many_arguments)]
pub fn calculate_quote_hash(
    chain_id: [u8; 32],
    attestation_id: [u8; 16],
    side: u8,
    user: Pubkey,
    asset: Pubkey,
    price: u64,
    amount: u64,
    expiration: i64,
) -> [u8; 32] {
    // Concatenate:
    //   chain_id (32)
    // + attestation_id (16)
    // + side (1)
    // + user (32)
    // + asset (32)
    // + price (8)
    // + amount (8)
    // + expiration (8) = 137 bytes
    let mut quote = [0u8; 137];
    quote[0..32].copy_from_slice(&chain_id);
    quote[32..48].copy_from_slice(&attestation_id);
    quote[48] = side;
    quote[49..81].copy_from_slice(&user.to_bytes());
    quote[81..113].copy_from_slice(&asset.to_bytes());
    quote[113..121].copy_from_slice(&price.to_be_bytes());
    quote[121..129].copy_from_slice(&amount.to_be_bytes());
    quote[129..137].copy_from_slice(&expiration.to_be_bytes());

    // Calculate keccak256 hash of the quote
    hash(&quote).to_bytes()
}

/// Verifies that the immediately preceding top-level instruction is a matching secp256k1 check.
/// # Arguments
/// * `ix_sysvar` - The instructions sysvar account info.
/// * `expected_digest32` - The expected 32-byte digest.
/// * `expected_eth_address20` - The expected 20-byte Ethereum address.
/// # Returns
/// * `Result<()>` - Ok if the instruction is found and matches, Err otherwise.
pub fn verify_secp256k1_ix(
    ix_sysvar: &AccountInfo,
    expected_digest32: &[u8; 32],
    expected_eth_address20: [u8; 20],
) -> Result<()> {
    let current_ix_idx = instructions::load_current_index_checked(ix_sysvar)?;

    require_gt!(current_ix_idx, 0, OndoError::MissingOrMismatchedSecpIx);

    let ix_idx = current_ix_idx - 1;

    let secp_ix = instructions::load_instruction_at_checked(ix_idx as usize, ix_sysvar)?;

    require_keys_eq!(
        secp_ix.program_id,
        secp256k1_program::id(),
        OndoError::MissingOrMismatchedSecpIx
    );

    secp_matches(
        ix_idx as u8,
        &secp_ix,
        expected_digest32,
        expected_eth_address20,
    )?;

    Ok(())
}

/// secp_matches checks if the given secp256k1 instruction matches the expected digest and Ethereum address.
/// The offsets point to a signature (64 bytes plus recovery ID), Ethereum address (20 bytes),
/// and message digest (32 bytes), all in the same instruction.
/// # Arguments
/// * `ix_idx` - The secp256k1 instruction's index in the transaction.
/// * `ix` - The instruction to parse.
/// * `digest` - The expected 32-byte digest.
/// * `eth_addr` - The expected 20-byte Ethereum address.
/// # Returns
/// * `Result<bool>` - Ok(true) if the instruction matches, Err otherwise.
fn secp_matches(
    ix_idx: u8,
    ix: &Instruction,
    digest: &[u8; 32],
    eth_addr: [u8; 20],
) -> Result<bool> {
    let data = &ix.data;

    // First byte is number of signatures; require 1 for this simple flow.
    require!(!data.is_empty(), OndoError::MalformedSecpIx);
    require!(data[0] == 1, OndoError::WrongSigCount);

    // Skip the header and parse the first signature offsets (see program docs for exact layout).
    // Extract the 20-byte Ethereum address and 32-byte message digest.
    // Offset structure is 11 bytes starting at byte 1:
    // [sig_off(2), sig_ix(1), eth_off(2), eth_ix(1), msg_off(2), msg_len(2), msg_ix(1)]
    let rd = 1;
    require!(data.len() >= rd + 11, OndoError::MalformedSecpIx);

    // parse instruction data
    let sig_ix = data[rd + 2];
    let eth_off = u16::from_le_bytes([data[rd + 3], data[rd + 4]]) as usize;
    let eth_ix = data[rd + 5];
    let msg_off = u16::from_le_bytes([data[rd + 6], data[rd + 7]]) as usize;
    let msg_len = u16::from_le_bytes([data[rd + 8], data[rd + 9]]) as usize;
    let msg_ix = data[rd + 10];

    require!(msg_len == 32, OndoError::WrongDigestLen);
    require!(msg_off + msg_len <= data.len(), OndoError::MalformedSecpIx);
    require!(eth_off + 20 <= data.len(), OndoError::MalformedSecpIx);
    // only support "inline" mode, the instruction must refer to itself for the calldata.
    // that is, the KeccakSecp256k11111111111111111111111111111 instruction must contain the signature, eth_address, and msg
    require!(sig_ix == ix_idx, OndoError::MissingOrMismatchedSecpIx);
    require!(eth_ix == ix_idx, OndoError::MissingOrMismatchedSecpIx);
    require!(msg_ix == ix_idx, OndoError::MissingOrMismatchedSecpIx);

    let msg = &data[msg_off..msg_off + 32];
    let eth_addr_in_ix = &data[eth_off..eth_off + 20];

    // The secp256k1 precompile has already verified:
    // 1. signature is valid for keccak256(msg) where msg is the 32-byte digest
    // 2. The signature recovers to the ETH address embedded in that instruction
    // We just need to verify:
    // - msg (the digest in the secp instruction) matches our calculated digest
    // - ETH address in the instruction matches our expected ETH address
    require!(msg == digest, OndoError::DigestMismatch);
    require!(eth_addr_in_ix == eth_addr, OndoError::AddressMismatch);

    Ok(true)
}
