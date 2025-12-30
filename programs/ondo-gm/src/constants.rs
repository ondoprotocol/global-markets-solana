use anchor_lang::prelude::*;

/// CHAIN IDS (Solana genesis hashes)
/// Mainnet-beta genesis hash
#[cfg(any(feature = "mainnet", feature = "testnet"))]
pub const CHAIN_ID: Pubkey = pubkey!("5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d");
/// Devnet genesis hash
#[cfg(feature = "devnet")]
pub const CHAIN_ID: Pubkey = pubkey!("EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG");
/// Localnet genesis hash (uses devnet)
#[cfg(feature = "localnet")]
pub const CHAIN_ID: Pubkey = pubkey!("EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG");
/// Default genesis hash (devnet)
#[cfg(not(any(
    feature = "mainnet",
    feature = "testnet",
    feature = "devnet",
    feature = "localnet"
)))]
pub const CHAIN_ID: Pubkey = pubkey!("EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG");

#[cfg(feature = "mainnet")]
pub const USDC_MINT: Pubkey = pubkey!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
#[cfg(feature = "testnet")]
pub const USDC_MINT: Pubkey = pubkey!("3Kyt2oSUoz3gKZNDpCptnW2URTX3ddp9nT1ytAwmUEaF");

/// 24 * 60 * 60 - Number of seconds in a day
pub const SECONDS_PER_DAY: i64 = 86400; // 24 * 60 * 60
/// 60 * 60 - Number of seconds in an hour
pub const SECONDS_PER_HOUR: i64 = 3600; // 60 * 60

// PDA SEEDS

/// Seed for the mint authority PDA
pub const MINT_AUTHORITY_SEED: &[u8] = b"mint_authority";
/// Seed for OndoUser account PDA
pub const ONDO_USER_SEED: &[u8] = b"OndoUser";
/// Seed for TokenLimit account PDA
pub const TOKEN_LIMIT_ACCOUNT_SEED: &[u8] = b"token";
/// Seed for USDonManagerState PDA
pub const USDON_MANAGER_STATE_SEED: &[u8] = b"usdon_manager";
/// Seed for GmTokenManagerState PDA
pub const GMTOKEN_MANAGER_STATE_SEED: &[u8] = b"gmtoken_manager";
/// Seed for Whitelist account PDA
pub const WHITELIST_SEED: &[u8] = b"whitelist";
/// Seed for oracle sanity check PDA
pub const ORACLE_SANITY_CHECK_SEED: &[u8] = b"sanity_check";
/// Seed for attestation ID PDA
pub const ATTESTATION_ID_SEED: &[u8] = b"attestation_id";

/// Pyth price feed ID for USDC/USD
pub const USDC_PYTH_ID: &str = "eaa020c61cc479712813461ce153894a96a6c00b21ed0cfc2798d1f9a9e9c94a";
pub const USDC_PYTH_ORACLE_ADDRESS: Pubkey =
    pubkey!("Dpw1EAVrSB1ibxiDQyTAW6Zip3J4Btk2x4SgApQCeFbX");

/// Minimum price threshold for USDC (in scaled units)
pub const MIN_PRICE: u64 = 98_000_000;
pub const USDC_PRICE_DECIMALS: u8 = 8;
pub const MAX_AGE_UPPER_BOUND: u64 = SECONDS_PER_DAY as u64;

/// Maximum allowed price delay
pub const MAX_SECONDS_EXPIRATION: i64 = 365 * SECONDS_PER_DAY;

/// Default attestation expiration time in seconds
pub const MAX_ATTESTATION_EXPIRATION: i64 = 30;

/// Default rate limit window in seconds (1 hour)
pub const DEFAULT_LIMIT_WINDOW: u64 = 3600;
/// Buy side identifier for attestations
pub const BUY: u8 = 0x30;
/// Sell side identifier for attestations
pub const SELL: u8 = 0x31;

/// Number of decimals for GM Token
pub const GM_TOKEN_DECIMALS: u8 = 9;

// SCALING FACTORS

/// 10^9 - Scaling factor for price calculations
pub const PRICE_SCALING_FACTOR: i64 = 1_000_000_000;
/// 10,000 basis points = 100% - Divisor for basis point calculations
pub const BASIS_POINTS_DIVISOR: u64 = 10_000;

pub const CONFIDENCE_THRESHOLD: u128 = 1;

/// The maximum amount of tokens that can be minted in a single admin mint operation
/// 10,000,000,000,000,000 units = 10 million tokens with 9 decimals
pub const MAX_MINT_AMOUNT: u64 = 10_000_000_000_000_000;

/// The maximum length for a token symbol
pub const SYMBOL_MAX_LENGTH: usize = 19;

/// The maximum length for a token name or URI
pub const NAME_AND_URI_MAX_LENGTH: usize = 256;
