use anchor_lang::prelude::*;

/// Roles state account - tracks role assignments for addresses
#[account]
pub struct Roles {
    // The address assigned to the role
    pub address: Pubkey,

    // The type of role assigned
    pub role: RoleType,

    // The bump used to derive the PDA for this account
    // Stored so we don't need to recalculate it later
    pub bump: u8,
}

impl Space for Roles {
    const INIT_SPACE: usize = 8 + size_of::<Roles>();
}

#[derive(Clone, Copy, PartialEq, Eq, AnchorDeserialize, AnchorSerialize)]
pub enum RoleType {
    MinterRoleUSDon,
    BurnerRoleUSDon,
    AdminRoleUSDon,
    AdminRoleUSDonManager,
    GuardianUSDon,
    DeployerRoleGMTokenFactory,
    PauserRoleGMTokenFactory,
    AdminRoleGMTokenFactory,
    MinterRoleGMToken,
    AdminRoleGMToken,
    PauserRoleGMTokenManager,
    PauserRoleGMToken,
    UnpauserRoleGMToken,
    AdminRoleGMTokenManager,
    IssuanceHoursRole,
    SetterRoleOndoSanityCheck,
    ConfigurerRoleOndoSanityCheck,
    AdminRoleOndoSanityCheck,
    AdminRoleWhitelist,
    UpdateMultiplierRole,
    UpdateMetadataRole,
}

impl RoleType {
    pub const MINTER_ROLE_USDON: &[u8] = b"MinterRoleUSDon";
    pub const BURNER_ROLE_USDON: &[u8] = b"BurnerRoleUSDon";
    pub const ADMIN_ROLE_USDON: &[u8] = b"AdminRoleUSDon";
    pub const ADMIN_ROLE_USDON_MANAGER: &[u8] = b"AdminRoleUSDonManager";
    pub const GUARDIAN_USDON: &[u8] = b"GuardianUSDon";
    pub const DEPLOYER_ROLE_GMTOKEN_FACTORY: &[u8] = b"DeployerRoleGMTokenFactory";
    pub const PAUSER_ROLE_GMTOKEN_FACTORY: &[u8] = b"PauserRoleGMTokenFactory";
    pub const ADMIN_ROLE_GMTOKEN_FACTORY: &[u8] = b"AdminRoleGMTokenFactory";
    pub const MINTER_ROLE_GMTOKEN: &[u8] = b"MinterRoleGMToken";
    pub const ADMIN_ROLE_GMTOKEN: &[u8] = b"AdminRoleGMToken";
    pub const PAUSER_ROLE_GMTOKEN: &[u8] = b"PauserRoleGMToken";
    pub const UNPAUSER_ROLE_GMTOKEN: &[u8] = b"UnpauserRoleGMToken";
    pub const PAUSER_ROLE_GMTOKEN_MANAGER: &[u8] = b"PauserRoleGMTokenManager";
    pub const ADMIN_ROLE_GMTOKEN_MANAGER: &[u8] = b"AdminRoleGMTokenManager";
    pub const ISSUANCE_HOURS_ROLE: &[u8] = b"IssuanceHoursRole";
    pub const SETTER_ROLE_ONDO_SANITY_CHECK: &[u8] = b"SetterRoleOndoSanityCheck";
    pub const CONFIGURER_ROLE_ONDO_SANITY_CHECK: &[u8] = b"ConfigurerRoleOndoSanityCheck";
    pub const ADMIN_ROLE_ONDO_SANITY_CHECK: &[u8] = b"AdminRoleOndoSanityCheck";
    pub const ADMIN_ROLE_WHITELIST: &[u8] = b"AdminRoleWhitelist";
    pub const UPDATE_MULTIPLIER_ROLE: &[u8] = b"UpdateMultiplierRole";

    pub const UPDATE_METADATA_ROLE: &[u8] = b"UpdateMetadataRole";

    pub const fn seed(&self) -> &'static [u8] {
        match self {
            RoleType::MinterRoleUSDon => Self::MINTER_ROLE_USDON,
            RoleType::BurnerRoleUSDon => Self::BURNER_ROLE_USDON,
            RoleType::AdminRoleUSDon => Self::ADMIN_ROLE_USDON,
            RoleType::AdminRoleUSDonManager => Self::ADMIN_ROLE_USDON_MANAGER,
            RoleType::GuardianUSDon => Self::GUARDIAN_USDON,
            RoleType::DeployerRoleGMTokenFactory => Self::DEPLOYER_ROLE_GMTOKEN_FACTORY,
            RoleType::PauserRoleGMTokenFactory => Self::PAUSER_ROLE_GMTOKEN_FACTORY,
            RoleType::AdminRoleGMTokenFactory => Self::ADMIN_ROLE_GMTOKEN_FACTORY,
            RoleType::MinterRoleGMToken => Self::MINTER_ROLE_GMTOKEN,
            RoleType::AdminRoleGMToken => Self::ADMIN_ROLE_GMTOKEN,
            RoleType::PauserRoleGMTokenManager => Self::PAUSER_ROLE_GMTOKEN_MANAGER,
            RoleType::PauserRoleGMToken => Self::PAUSER_ROLE_GMTOKEN,
            RoleType::UnpauserRoleGMToken => Self::UNPAUSER_ROLE_GMTOKEN,
            RoleType::AdminRoleGMTokenManager => Self::ADMIN_ROLE_GMTOKEN_MANAGER,
            RoleType::IssuanceHoursRole => Self::ISSUANCE_HOURS_ROLE,
            RoleType::SetterRoleOndoSanityCheck => Self::SETTER_ROLE_ONDO_SANITY_CHECK,
            RoleType::ConfigurerRoleOndoSanityCheck => Self::CONFIGURER_ROLE_ONDO_SANITY_CHECK,
            RoleType::AdminRoleOndoSanityCheck => Self::ADMIN_ROLE_ONDO_SANITY_CHECK,
            RoleType::AdminRoleWhitelist => Self::ADMIN_ROLE_WHITELIST,
            RoleType::UpdateMultiplierRole => Self::UPDATE_MULTIPLIER_ROLE,
            RoleType::UpdateMetadataRole => Self::UPDATE_METADATA_ROLE,
        }
    }
}
