use anchor_lang::prelude::*;

use crate::{
    constants::GMTOKEN_MANAGER_STATE_SEED,
    errors::OndoError,
    events::{RoleGranted, RoleRevoked},
    state::{GMTokenManagerState, RoleType, Roles},
};

/// Initialize the `GMTokenManagerState` account
/// Requires `ADMIN_ROLE_GMTOKEN_MANAGER` role
#[derive(Accounts)]
pub struct InitializeGMTokenManager<'info> {
    /// Pays for account creation
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The account with the authority to initialize the GM Token Manager
    pub authority: Signer<'info>,

    /// The `GMTokenManagerState` account to be initialized
    /// # PDA Seeds
    /// - `GMTOKEN_MANAGER_STATE_SEED`
    #[account(
        init,
        payer = payer,
        space = 8 + GMTokenManagerState::INIT_SPACE,
        seeds = [GMTOKEN_MANAGER_STATE_SEED],
        bump
    )]
    pub gmtoken_manager_state: Account<'info, GMTokenManagerState>,

    /// The `Roles` account verifying the authority has the `ADMIN_ROLE_GMTOKEN_MANAGER` role
    /// # PDA Seeds
    /// - `ADMIN_ROLE_GMTOKEN_MANAGER`
    /// - The authority's address
    #[account(
        seeds = [RoleType::ADMIN_ROLE_GMTOKEN_MANAGER, authority.key().as_ref()],
        bump = authority_role_account.bump,
    )]
    pub authority_role_account: Account<'info, Roles>,

    /// The system program
    pub system_program: Program<'info, System>,
}

impl<'info> InitializeGMTokenManager<'info> {
    /// Initialize the `GMTokenManagerState` account
    /// # Arguments
    /// * `factory_paused` - Whether the GM Token factory should start in a paused state
    /// * `redemption_paused` - Initial global redemption pause flag
    /// * `minting_paused` - Initial global minting pause flag
    /// * `attestation_signer_secp` - The secp256k1 Ethereum address of the attestation signer (20 bytes)
    /// * `trading_hours_offset` - Legacy UTC offset in seconds, stored but not used to gate trades
    /// * `bumps` - The PDA bumps for account derivation
    /// # Returns
    /// * `Result<()>` - Ok if the GMTokenManagerState is successfully initialized, Err otherwise
    pub fn initialize_gmtoken_manager(
        &mut self,
        factory_paused: bool,
        redemption_paused: bool,
        minting_paused: bool,
        attestation_signer_secp: [u8; 20],
        trading_hours_offset: i64,
        bumps: &InitializeGMTokenManagerBumps,
    ) -> Result<()> {
        self.gmtoken_manager_state.set_inner(GMTokenManagerState {
            execution_id: None,
            factory_paused,
            redemption_paused,
            minting_paused,
            bump: bumps.gmtoken_manager_state,
            attestation_signer_secp,
            trading_hours_offset,
        });

        Ok(())
    }
}

/// Update the configured secp256k1 attestation signer using `AdminRoleGMTokenManager`.
#[derive(Accounts)]
pub struct GMTokenManagerSetSigner<'info> {
    /// Administrator authorizing the attestation signer change.
    pub authority: Signer<'info>,

    /// The `Roles` account verifying the authority has the `ADMIN_ROLE_GMTOKEN_MANAGER` role
    /// # PDA Seeds
    /// - `ADMIN_ROLE_GMTOKEN_MANAGER`
    /// - The authority's address
    #[account(
        seeds = [RoleType::ADMIN_ROLE_GMTOKEN_MANAGER, authority.key().as_ref()],
        bump = authority_role_account.bump,
    )]
    pub authority_role_account: Account<'info, Roles>,

    /// The `GMTokenManagerState` account to be modified
    /// # PDA Seeds
    /// - `GMTOKEN_MANAGER_STATE_SEED`
    #[account(
        mut,
        seeds = [GMTOKEN_MANAGER_STATE_SEED],
        bump = gmtoken_manager_state.bump,
    )]
    pub gmtoken_manager_state: Account<'info, GMTokenManagerState>,
}

impl<'info> GMTokenManagerSetSigner<'info> {
    /// Set the attestation signer secp256k1 Ethereum address
    /// # Arguments
    /// * `attestation_signer_secp` - The new secp256k1 Ethereum address of the attestation signer (20 bytes)
    pub fn set_attestation_signer_secp(&mut self, attestation_signer_secp: [u8; 20]) -> Result<()> {
        // Update the attestation signer address
        self.gmtoken_manager_state.attestation_signer_secp = attestation_signer_secp;

        Ok(())
    }
}

/// Grant a GM Token Manager role to a user by initializing a `Roles` account
/// Requires `ADMIN_ROLE_GMTOKEN_MANAGER` role
/// Only the `PauserRoleGMTokenManager` role can be added
#[derive(Accounts)]
#[instruction(role: RoleType, user: Pubkey)]
pub struct GMTokenManagerGrantRole<'info> {
    /// Pays for account creation
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The account with the authority to grant GM Token Manager roles
    pub authority: Signer<'info>,

    /// The `Roles` account verifying the authority has the `ADMIN_ROLE_GMTOKEN_MANAGER` role
    /// # PDA Seeds
    /// - ADMIN_ROLE_GMTOKEN_MANAGER
    /// - The authority's address
    #[account(
        seeds = [RoleType::ADMIN_ROLE_GMTOKEN_MANAGER, authority.key().as_ref()],
        bump = authority_role_account.bump,
    )]
    pub authority_role_account: Account<'info, Roles>,

    /// The new `Roles` account being created for the user
    /// # PDA Seeds
    /// - Role seed (from RoleType)
    /// - User's address
    #[account(
        init,
        payer = payer,
        space = Roles::INIT_SPACE,
        seeds = [role.seed(), user.as_ref()],
        bump
    )]
    pub role_to_grant: Account<'info, Roles>,

    /// The system program
    pub system_program: Program<'info, System>,
}

impl<'info> GMTokenManagerGrantRole<'info> {
    /// Grant `PauserRoleGMTokenManager` and emit `RoleGranted`.
    /// # Arguments
    /// * `role` - The role to grant (must be `PauserRoleGMTokenManager`)
    /// * `user` - The public key of the user to grant the role to
    /// * `bumps` - The PDA bumps for account derivation
    /// # Returns
    /// * `Result<()>` - Ok if the role is successfully granted, Err otherwise
    pub fn grant_gmtoken_manager_role(
        &mut self,
        role: RoleType,
        user: Pubkey,
        bumps: &GMTokenManagerGrantRoleBumps,
    ) -> Result<()> {
        // Only allow PauserRoleGMTokenManager roles to be created
        require!(
            matches!(role, RoleType::PauserRoleGMTokenManager),
            OndoError::InvalidRoleType
        );

        // Write to the new Roles account
        self.role_to_grant.address = user;
        self.role_to_grant.role = role;
        self.role_to_grant.bump = bumps.role_to_grant;

        // Emit event for role granted
        emit!(RoleGranted {
            role,
            grantee: user,
            granter: self.authority.key(),
        });

        Ok(())
    }
}

/// Revoke a GM Token Manager role from a user by closing their `Roles` account
/// Requires `ADMIN_ROLE_GMTOKEN_MANAGER` role
/// Only the `PauserRoleGMTokenManager` role can be removed
#[derive(Accounts)]
pub struct GMTokenManagerRevokeRole<'info> {
    /// The account with the authority to revoke GM Token Manager roles
    pub authority: Signer<'info>,

    /// Receives the lamports from closing the Roles account
    #[account(mut)]
    pub recipient: SystemAccount<'info>,

    /// The `Roles` account verifying the authority has the `ADMIN_ROLE_GMTOKEN_MANAGER` role
    /// # PDA Seeds
    /// - `ADMIN_ROLE_GMTOKEN_MANAGER`
    /// - The authority's address
    #[account(
        seeds = [RoleType::ADMIN_ROLE_GMTOKEN_MANAGER, authority.key().as_ref()],
        bump = authority_role_account.bump,
    )]
    pub authority_role_account: Account<'info, Roles>,

    /// The Roles account being closed
    /// # PDA Seeds
    /// - `role_to_revoke.role.seed()` (the seed for the role)
    /// - `role_to_revoke.address` (the user's address)
    #[account(
        mut,
        close = recipient,
        seeds = [
            role_to_revoke.role.seed(),
            role_to_revoke.address.as_ref()
        ],
        bump = role_to_revoke.bump,
    )]
    pub role_to_revoke: Account<'info, Roles>,

    /// The system program
    pub system_program: Program<'info, System>,
}

impl<'info> GMTokenManagerRevokeRole<'info> {
    /// Revoke `PauserRoleGMTokenManager`, refund rent to `recipient`, and emit `RoleRevoked`.
    pub fn revoke_gmtoken_manager_role(&mut self) -> Result<()> {
        // Only allow PauserRoleGMTokenManager roles to be revoked
        require!(
            matches!(self.role_to_revoke.role, RoleType::PauserRoleGMTokenManager),
            OndoError::InvalidRoleType
        );

        // Emit event for role revoked
        emit!(RoleRevoked {
            role: self.role_to_revoke.role,
            grantee: self.role_to_revoke.address,
            revoker: self.authority.key(),
        });

        Ok(())
    }
}
