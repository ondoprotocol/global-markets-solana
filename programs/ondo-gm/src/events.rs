use anchor_lang::prelude::*;

use crate::state::RoleType;

/// Event emitted when a role is granted to a user
/// Fields:
/// - role: The role that was granted
/// - grantee: The public key of the user who was granted the role
/// - granter: The public key of the user who granted the role
#[event]
pub struct RoleGranted {
    pub role: RoleType,
    pub grantee: Pubkey,
    pub granter: Pubkey,
}

/// Event emitted when a role is revoked from a user
/// Fields:
/// - role: The role that was revoked
/// - grantee: The public key of the user who had the role revoked
/// - revoker: The public key of the user who revoked the role
#[event]
pub struct RoleRevoked {
    pub role: RoleType,
    pub grantee: Pubkey,
    pub revoker: Pubkey,
}

/// Event emitted when a rate limit is set for a user
/// Fields:
/// - user: The public key of the user for whom the rate limit is set
/// - limit: USD notional capacity per direction, scaled to 9 decimals
#[event]
pub struct RateLimitUserSet {
    pub user: Pubkey,
    pub limit: u64,
}

/// Event emitted when a rate limit is set for a token
/// Fields:
/// - token: The public key of the token for which the rate limit is set
/// - limit: USD notional capacity per direction, scaled to 9 decimals
/// - limit_window: The time window for the rate limit
#[event]
pub struct RateLimitTokenSet {
    pub token: Pubkey,
    pub limit: Option<u64>,
    pub limit_window: Option<u64>,
}

/// Event emitted when a sanity check is set for a mint
/// Fields:
/// - mint: The public key of the mint for which the sanity check is set
/// - allowed_deviation_bps: The allowed deviation in basis points
/// - max_time_delay: The maximum time delay for the sanity check
#[event]
pub struct SanityCheckSet {
    pub mint: Pubkey,
    pub allowed_deviation_bps: u64,
    pub max_time_delay: i64,
}

/// Event emitted when a sanity check is updated for a mint
/// Fields:
/// - mint: The public key of the mint for which the sanity check is updated
/// - last_price: The last recorded price (optional)
/// - allowed_deviation_bps: The allowed deviation in basis points (optional)
/// - max_time_delay: The maximum time delay for the sanity check (optional)
#[event]
pub struct SanityCheckUpdated {
    pub mint: Pubkey,
    pub last_price: Option<u64>,
    pub allowed_deviation_bps: Option<u64>,
    pub max_time_delay: Option<i64>,
}

/// Event emitted when a token is deployed
/// Fields:
/// - mint: The public key of the deployed token
/// - kind: Token family: 0 for GM/USDon, 1 for portfolio
#[event]
pub struct TokenDeployed {
    pub mint: Pubkey,
    pub kind: u8,
}

/// Event emitted when the Token Factory is paused or unpaused
/// Fields:
/// - is_paused: Boolean indicating if the Token Factory is paused
/// - pauser: The address of the operator who performed the pause/unpause action
/// - kind: Token family: 0 for GM/USDon, 1 for portfolio
#[event]
pub struct TokenFactoryPaused {
    pub is_paused: bool,
    pub pauser: Pubkey,
    pub kind: u8,
}

/// Event emitted when token minting is globally paused or unpaused
/// Fields:
/// - is_paused: Boolean indicating if the minting is globally paused
/// - pauser: The address of the user who performed the pause/unpause action
/// - kind: Token family: 0 for GM/USDon, 1 for portfolio
#[event]
pub struct TokenManagerMintingPaused {
    pub is_paused: bool,
    pub pauser: Pubkey,
    pub kind: u8,
}

/// Event emitted when token redemptions are globally paused or unpaused
/// Fields:
/// - is_paused: Boolean indicating if redemptions are globally paused
/// - pauser: The address of the operator who performed the pause/unpause action
/// - kind: Token family: 0 for GM/USDon, 1 for portfolio
#[event]
pub struct TokenManagerRedemptionPaused {
    pub is_paused: bool,
    pub pauser: Pubkey,
    pub kind: u8,
}

/// Event emitted when minting for a token is paused or unpaused
/// Fields:
/// - is_paused: Boolean indicating if minting is paused
/// - token: The mint address
/// - pauser: The address of the operator who performed the pause/unpause action
#[event]
pub struct TokenMintingPaused {
    pub is_paused: bool,
    pub token: Pubkey,
    pub pauser: Pubkey,
}

/// Event emitted when per-mint redemptions are paused or unpaused
/// Fields:
/// - is_paused: Boolean indicating if redemptions are paused
/// - token: The address of the token
/// - pauser: The address of the operator who performed the pause/unpause action
#[event]
pub struct TokenRedemptionPaused {
    pub is_paused: bool,
    pub token: Pubkey,
    pub pauser: Pubkey,
}

/// Event emitted when a Token is paused/unpaused at the mint-level
/// Fields:
/// - is_paused: Boolean indicating whether the token is paused
/// - token: The mint address
/// - pauser: The address of the operator who performed the pause/unpaused action
/// - kind: Token family: 0 for GM/USDon, 1 for portfolio
#[event]
pub struct TokenPaused {
    pub is_paused: bool,
    pub token: Pubkey,
    pub pauser: Pubkey,
    pub kind: u8,
}

/// Event emitted when a trade is executed
/// Fields:
/// - execution_id: Monotonically increasing ID within the GM or portfolio manager
///   sequence; use the invoking instruction to identify the token family
#[event]
pub struct TradeExecuted {
    pub execution_id: u128,
}

/// Event emitted when tokens are retrieved (withdrawn) from a vault by an admin
/// Fields:
/// - token: The public key of the token mint being withdrawn
/// - to: The destination address receiving the tokens
/// - amount: The amount of tokens withdrawn
/// - authority: The public key of the admin who executed the withdrawal
#[event]
pub struct TokensRetrieved {
    pub token: Pubkey,
    pub to: Pubkey,
    pub amount: u64,
    pub authority: Pubkey,
}

/// Event emitted when a user is added to the whitelist
/// Fields:
/// - user: The public key of the user being added to the whitelist
/// - added_by: The public key of the admin who added the user to the whitelist
#[event]
pub struct UserAddedToWhitelist {
    pub user: Pubkey,
    pub added_by: Pubkey,
}

/// Event emitted when a user is removed from the whitelist
/// Fields:
/// - user: The public key of the user being removed from the whitelist
/// - removed_by: The public key of the admin who removed the user from the whitelist
#[event]
pub struct UserRemovedFromWhitelist {
    pub user: Pubkey,
    pub removed_by: Pubkey,
}
