use anchor_lang::prelude::*;

/// One role assignment, stored at the PDA `[role.seed(), address]`.
///
/// An address can hold several roles through separate accounts. Role assignments
/// are program-wide: the PDA contains no mint, user-limit, or portfolio scope.
/// Each instruction separately constrains which accounts the holder may modify.
#[account]
pub struct Roles {
    /// Address that must authorize instructions using this role.
    pub address: Pubkey,

    /// Permission assigned to the address.
    pub role: RoleType,

    /// Canonical bump for `[role.seed(), address]`.
    pub bump: u8,
}

impl Space for Roles {
    const INIT_SPACE: usize = 8 + size_of::<Roles>();
}

/// Permissions enforced by instruction account constraints.
///
/// The program upgrade authority can grant or revoke every variant through
/// `grant_role` and `revoke_role`. Where a delegated administrator is listed
/// below, it can also grant and revoke that role through its dedicated instructions.
/// Administrator roles do not implicitly inherit the permissions of other roles.
/// Variant order is part of the serialized account format and must remain stable.
#[derive(Clone, Copy, PartialEq, Eq, AnchorDeserialize, AnchorSerialize)]
pub enum RoleType {
    /// Mint USDon directly with `mint_usdon`.
    /// Delegated administrator: `GuardianUSDon`.
    MinterRoleUSDon,

    /// Burn USDon from a token account through the mint's permanent delegate.
    /// Delegated administrator: `GuardianUSDon`.
    BurnerRoleUSDon,

    /// Mint and burn USDon directly. Does not administer role assignments.
    /// Granted and revoked only by the program upgrade authority.
    AdminRoleUSDon,

    /// Configure the USDC oracle guard and withdraw tokens from manager vaults.
    /// Granted and revoked only by the program upgrade authority.
    AdminRoleUSDonManager,

    /// Initialize the USDon manager and grant or revoke its minter and burner roles.
    /// Granted and revoked only by the program upgrade authority.
    GuardianUSDon,

    /// Deprecated: no instruction accepts this role. Retained to preserve enum
    /// discriminants and existing role PDAs. Use `DeployerRoleTokenFactory` instead.
    DeployerRoleGMTokenFactory,

    /// Pause factory mint creation. Resuming requires `AdminRoleGMTokenFactory`.
    /// Delegated administrator: `AdminRoleGMTokenFactory`.
    PauserRoleGMTokenFactory,

    /// Pause or resume factory mint creation and manage the `PauserRoleGMTokenFactory` role.
    /// Granted and revoked only by the program upgrade authority.
    AdminRoleGMTokenFactory,

    /// Mint GM tokens directly through `mint_gm`, subject to its minting limits.
    /// Delegated administrator: `AdminRoleGMToken`.
    MinterRoleGMToken,

    /// Grant or revoke `MinterRoleGMToken`, `PauserRoleGMToken`, and
    /// `UnpauserRoleGMToken`. This role does not itself mint, pause, or resume tokens.
    /// Granted and revoked only by the program upgrade authority.
    AdminRoleGMToken,

    /// Pause GM minting/redemption globally.
    /// Resuming requires `AdminRoleGMTokenManager`.
    /// Delegated administrator: `AdminRoleGMTokenManager`.
    PauserRoleGMTokenManager,

    /// Pause a GM mint through Token-2022's Pausable extension, blocking token
    /// minting, burning, and transfers. Does not set manager or `TokenLimit` flags.
    /// Delegated administrator: `AdminRoleGMToken`.
    PauserRoleGMToken,

    /// Resume a GM mint through Token-2022's Pausable extension. Manager and
    /// `TokenLimit` pause flags must be cleared separately to restore trading.
    /// Delegated administrator: `AdminRoleGMToken`.
    UnpauserRoleGMToken,

    /// Initialize/configure the GM manager and its attestation signer; pause/resume global GM trading;
    /// and manage `PauserRoleGMTokenManager` assignments.
    /// Granted and revoked only by the program upgrade authority.
    AdminRoleGMTokenManager,

    /// Deprecated: no instruction accepts this role. Retained to preserve enum
    /// discriminants and existing role PDAs. Trading hours are no longer enforced on-chain.
    IssuanceHoursRole,

    /// Set a mint's last sanity-check price and refresh its observation timestamp.
    /// Delegated administrator: `AdminRoleOndoSanityCheck`.
    SetterRoleOndoSanityCheck,

    /// Configure a mint's permitted price deviation and observation age.
    /// Delegated administrator: `AdminRoleOndoSanityCheck`.
    ConfigurerRoleOndoSanityCheck,

    /// Grant or revoke `SetterRoleOndoSanityCheck` and `ConfigurerRoleOndoSanityCheck`.
    /// Granted and revoked only by the program upgrade authority.
    AdminRoleOndoSanityCheck,

    /// Add or remove addresses from the protocol whitelist.
    /// Granted and revoked only by the program upgrade authority.
    AdminRoleWhitelist,

    /// Update a GM mint's Token-2022 scaled UI multiplier and effective timestamp.
    /// Portfolio mints and USDon are rejected.
    /// Granted and revoked only by the program upgrade authority.
    UpdateMultiplierRole,

    /// Update a protocol-controlled mint's name, symbol, and URI through Token-2022 metadata.
    /// Granted and revoked only by the program upgrade authority.
    UpdateMetadataRole,

    /// Pause portfolio mint creation. Resuming requires `AdminRolePortfolioTokenFactory`.
    /// Delegated administrator: `AdminRolePortfolioTokenFactory`.
    PauserRolePortfolioTokenFactory,

    /// Pause/resume portfolio mint creation and manage the portfolio factory pauser role.
    /// Granted and revoked only by the program upgrade authority.
    AdminRolePortfolioTokenFactory,

    /// Pause portfolio minting/redemption globally. Resuming requires the portfolio manager admin.
    /// Delegated administrator: `AdminRolePortfolioTokenManager`.
    PauserRolePortfolioTokenManager,

    /// Mint portfolio tokens directly through `mint_portfolio`, subject to its limits.
    /// Delegated administrator: `AdminRolePortfolioToken`.
    MinterRolePortfolioToken,

    /// Manage portfolio minter, mint pauser, and mint unpauser assignments.
    /// Does not itself mint, pause, or resume tokens.
    /// Granted and revoked only by the program upgrade authority.
    AdminRolePortfolioToken,

    /// Pause a portfolio mint through Token-2022, blocking minting, burning, and transfers.
    /// Delegated administrator: `AdminRolePortfolioToken`.
    PauserRolePortfolioToken,

    /// Resume a portfolio mint through Token-2022; manager and per-token flags remain independent.
    /// Delegated administrator: `AdminRolePortfolioToken`.
    UnpauserRolePortfolioToken,

    /// Initialize the portfolio manager, configure its attestation signer, pause/resume
    /// global portfolio trading, and manage its pauser role.
    /// Granted and revoked only by the program upgrade authority.
    AdminRolePortfolioTokenManager,

    /// Pause a token's minting/redemption through its `TokenLimit` account.
    /// Delegated administrator: `AdminRoleTokenLimit`.
    PauserRoleTokenLimit,

    /// Grant or revoke `PauserRoleTokenLimit`; pause or resume a token's minting/redemption
    /// through its `TokenLimit` account; and configure limits.
    /// Granted and revoked only by the program upgrade authority.
    AdminRoleTokenLimit,

    /// Set a user's minting/redemption limits for one mint through its `OndoUser` account.
    /// Can raise or lower a user's limits, so hold it only on a multisig that needs more than one
    /// human approval.
    /// Granted and revoked only by the program upgrade authority.
    AdminRoleUserLimit,

    /// Create Token-2022 mints and initialize their `TokenLimit` and `OracleSanityCheck` accounts.
    /// Mint creation remains subject to each factory's pause flag.
    /// Granted and revoked only by the program upgrade authority.
    DeployerRoleTokenFactory,

    /// Initialize a user's `OndoUser` account for one mint with that mint's `TokenLimit` default
    /// user limits, as the user's first trade would. Cannot set limits.
    /// Granted and revoked only by the program upgrade authority.
    InitializerRoleOndoUser,
}

impl RoleType {
    pub const MINTER_ROLE_USDON: &[u8] = b"MinterRoleUSDon";
    pub const BURNER_ROLE_USDON: &[u8] = b"BurnerRoleUSDon";
    pub const ADMIN_ROLE_USDON: &[u8] = b"AdminRoleUSDon";
    pub const ADMIN_ROLE_USDON_MANAGER: &[u8] = b"AdminRoleUSDonManager";
    pub const GUARDIAN_USDON: &[u8] = b"GuardianUSDon";
    pub const DEPLOYER_ROLE_GMTOKEN_FACTORY: &[u8] = b"DeployerRoleGMTokenFactory"; // DEPRECATED, kept for compat
    pub const PAUSER_ROLE_GMTOKEN_FACTORY: &[u8] = b"PauserRoleGMTokenFactory";
    pub const ADMIN_ROLE_GMTOKEN_FACTORY: &[u8] = b"AdminRoleGMTokenFactory";
    pub const MINTER_ROLE_GMTOKEN: &[u8] = b"MinterRoleGMToken";
    pub const ADMIN_ROLE_GMTOKEN: &[u8] = b"AdminRoleGMToken";
    pub const PAUSER_ROLE_GMTOKEN: &[u8] = b"PauserRoleGMToken";
    pub const UNPAUSER_ROLE_GMTOKEN: &[u8] = b"UnpauserRoleGMToken";
    pub const PAUSER_ROLE_GMTOKEN_MANAGER: &[u8] = b"PauserRoleGMTokenManager";
    pub const ADMIN_ROLE_GMTOKEN_MANAGER: &[u8] = b"AdminRoleGMTokenManager";
    pub const ISSUANCE_HOURS_ROLE: &[u8] = b"IssuanceHoursRole"; // DEPRECATED, kept for compat
    pub const SETTER_ROLE_ONDO_SANITY_CHECK: &[u8] = b"SetterRoleOndoSanityCheck";
    pub const CONFIGURER_ROLE_ONDO_SANITY_CHECK: &[u8] = b"ConfigurerRoleOndoSanityCheck";
    pub const ADMIN_ROLE_ONDO_SANITY_CHECK: &[u8] = b"AdminRoleOndoSanityCheck";
    pub const ADMIN_ROLE_WHITELIST: &[u8] = b"AdminRoleWhitelist";
    pub const UPDATE_MULTIPLIER_ROLE: &[u8] = b"UpdateMultiplierRole";
    pub const UPDATE_METADATA_ROLE: &[u8] = b"UpdateMetadataRole";
    pub const PAUSER_ROLE_PORTFOLIO_TOKEN_FACTORY: &[u8] = b"PauserRolePortfolioTokenFactory";
    pub const ADMIN_ROLE_PORTFOLIO_TOKEN_FACTORY: &[u8] = b"AdminRolePortfolioTokenFactory";
    pub const PAUSER_ROLE_PORTFOLIO_TOKEN_MANAGER: &[u8] = b"PauserRolePortfolioTokenManager";
    pub const MINTER_ROLE_PORTFOLIO_TOKEN: &[u8] = b"MinterRolePortfolioToken";
    pub const ADMIN_ROLE_PORTFOLIO_TOKEN: &[u8] = b"AdminRolePortfolioToken";
    pub const PAUSER_ROLE_PORTFOLIO_TOKEN: &[u8] = b"PauserRolePortfolioToken";
    pub const UNPAUSER_ROLE_PORTFOLIO_TOKEN: &[u8] = b"UnpauserRolePortfolioToken";
    pub const ADMIN_ROLE_PORTFOLIO_TOKEN_MANAGER: &[u8] = b"AdminRolePortfolioTokenManager";
    pub const PAUSER_ROLE_TOKEN_LIMIT: &[u8] = b"PauserRoleTokenLimit";
    pub const ADMIN_ROLE_TOKEN_LIMIT: &[u8] = b"AdminRoleTokenLimit";
    pub const ADMIN_ROLE_USER_LIMIT: &[u8] = b"AdminRoleUserLimit";
    pub const DEPLOYER_ROLE_TOKEN_FACTORY: &[u8] = b"DeployerRoleTokenFactory";
    pub const INITIALIZER_ROLE_ONDO_USER: &[u8] = b"InitializerRoleOndoUser";

    pub const fn seed(&self) -> &'static [u8] {
        match self {
            RoleType::MinterRoleUSDon => Self::MINTER_ROLE_USDON,
            RoleType::BurnerRoleUSDon => Self::BURNER_ROLE_USDON,
            RoleType::AdminRoleUSDon => Self::ADMIN_ROLE_USDON,
            RoleType::AdminRoleUSDonManager => Self::ADMIN_ROLE_USDON_MANAGER,
            RoleType::GuardianUSDon => Self::GUARDIAN_USDON,
            RoleType::DeployerRoleGMTokenFactory => Self::DEPLOYER_ROLE_GMTOKEN_FACTORY, // DEPRECATED, kept for compat
            RoleType::PauserRoleGMTokenFactory => Self::PAUSER_ROLE_GMTOKEN_FACTORY,
            RoleType::AdminRoleGMTokenFactory => Self::ADMIN_ROLE_GMTOKEN_FACTORY,
            RoleType::MinterRoleGMToken => Self::MINTER_ROLE_GMTOKEN,
            RoleType::AdminRoleGMToken => Self::ADMIN_ROLE_GMTOKEN,
            RoleType::PauserRoleGMTokenManager => Self::PAUSER_ROLE_GMTOKEN_MANAGER,
            RoleType::PauserRoleGMToken => Self::PAUSER_ROLE_GMTOKEN,
            RoleType::UnpauserRoleGMToken => Self::UNPAUSER_ROLE_GMTOKEN,
            RoleType::AdminRoleGMTokenManager => Self::ADMIN_ROLE_GMTOKEN_MANAGER,
            RoleType::IssuanceHoursRole => Self::ISSUANCE_HOURS_ROLE, // DEPRECATED, kept for compat
            RoleType::SetterRoleOndoSanityCheck => Self::SETTER_ROLE_ONDO_SANITY_CHECK,
            RoleType::ConfigurerRoleOndoSanityCheck => Self::CONFIGURER_ROLE_ONDO_SANITY_CHECK,
            RoleType::AdminRoleOndoSanityCheck => Self::ADMIN_ROLE_ONDO_SANITY_CHECK,
            RoleType::AdminRoleWhitelist => Self::ADMIN_ROLE_WHITELIST,
            RoleType::UpdateMultiplierRole => Self::UPDATE_MULTIPLIER_ROLE,
            RoleType::UpdateMetadataRole => Self::UPDATE_METADATA_ROLE,
            RoleType::PauserRolePortfolioTokenFactory => Self::PAUSER_ROLE_PORTFOLIO_TOKEN_FACTORY,
            RoleType::AdminRolePortfolioTokenFactory => Self::ADMIN_ROLE_PORTFOLIO_TOKEN_FACTORY,
            RoleType::PauserRolePortfolioTokenManager => Self::PAUSER_ROLE_PORTFOLIO_TOKEN_MANAGER,
            RoleType::MinterRolePortfolioToken => Self::MINTER_ROLE_PORTFOLIO_TOKEN,
            RoleType::AdminRolePortfolioToken => Self::ADMIN_ROLE_PORTFOLIO_TOKEN,
            RoleType::PauserRolePortfolioToken => Self::PAUSER_ROLE_PORTFOLIO_TOKEN,
            RoleType::UnpauserRolePortfolioToken => Self::UNPAUSER_ROLE_PORTFOLIO_TOKEN,
            RoleType::AdminRolePortfolioTokenManager => Self::ADMIN_ROLE_PORTFOLIO_TOKEN_MANAGER,
            RoleType::PauserRoleTokenLimit => Self::PAUSER_ROLE_TOKEN_LIMIT,
            RoleType::AdminRoleTokenLimit => Self::ADMIN_ROLE_TOKEN_LIMIT,
            RoleType::AdminRoleUserLimit => Self::ADMIN_ROLE_USER_LIMIT,
            RoleType::DeployerRoleTokenFactory => Self::DEPLOYER_ROLE_TOKEN_FACTORY,
            RoleType::InitializerRoleOndoUser => Self::INITIALIZER_ROLE_ONDO_USER,
        }
    }
}
