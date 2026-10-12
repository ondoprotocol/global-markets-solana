use anchor_lang::prelude::*;

use crate::{
    constants::*,
    errors::OndoError,
    events::{RoleGranted, RoleRevoked},
    state::{PortfolioTokenManagerState, RoleType, Roles},
};

/// Initialize the `PortfolioTokenManagerState` account
/// Requires `ADMIN_ROLE_PORTFOLIO_TOKEN_MANAGER` role
#[derive(Accounts)]
pub struct InitializePortfolioTokenManager<'info> {
    /// Pays for account creation
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The account with the authority to initialize the Portfolio Token Manager
    pub authority: Signer<'info>,

    /// The `PortfolioTokenManagerState` account to be initialized
    #[account(
        init,
        payer = payer,
        space = 8 + PortfolioTokenManagerState::INIT_SPACE,
        seeds = [PORTFOLIO_TOKEN_MANAGER_STATE_SEED],
        bump
    )]
    pub portfolio_token_manager: Account<'info, PortfolioTokenManagerState>,

    /// The `Roles` account verifying the authority has the `ADMIN_ROLE_PORTFOLIO_TOKEN_MANAGER` role
    #[account(
        seeds = [RoleType::ADMIN_ROLE_PORTFOLIO_TOKEN_MANAGER, authority.key().as_ref()],
        bump = authority_role_account.bump,
    )]
    pub authority_role_account: Account<'info, Roles>,

    /// The system program
    pub system_program: Program<'info, System>,
}

impl<'info> InitializePortfolioTokenManager<'info> {
    /// Initialize the `PortfolioTokenManagerState` account
    /// # Arguments
    /// * `factory_paused` - Whether the Portfolio Token factory should start in a paused state
    /// * `redemption_paused` - Whether redemptions should start in a paused state
    /// * `minting_paused` - Whether minting should start in a paused state
    /// * `attestation_signer_secp` - The secp256k1 Ethereum address of the attestation signer (20 bytes)
    pub fn initialize_portfolio_token_manager(
        &mut self,
        factory_paused: bool,
        redemption_paused: bool,
        minting_paused: bool,
        attestation_signer_secp: [u8; 20],
        bumps: &InitializePortfolioTokenManagerBumps,
    ) -> Result<()> {
        self.portfolio_token_manager
            .set_inner(PortfolioTokenManagerState {
                execution_id: None,
                factory_paused,
                redemption_paused,
                minting_paused,
                bump: bumps.portfolio_token_manager,
                attestation_signer_secp,
            });

        Ok(())
    }
}

/// Update the configured secp256k1 attestation signer using `AdminRolePortfolioTokenManager`.
#[derive(Accounts)]
pub struct PortfolioTokenManagerSetSigner<'info> {
    /// Administrator authorizing the attestation signer change.
    pub authority: Signer<'info>,

    /// The `Roles` account verifying the authority has the `ADMIN_ROLE_PORTFOLIO_TOKEN_MANAGER` role
    /// # PDA Seeds
    /// - `ADMIN_ROLE_PORTFOLIO_TOKEN_MANAGER`
    /// - The authority's address
    #[account(
        seeds = [RoleType::ADMIN_ROLE_PORTFOLIO_TOKEN_MANAGER, authority.key().as_ref()],
        bump = authority_role_account.bump,
    )]
    pub authority_role_account: Account<'info, Roles>,

    /// The `PortfolioTokenManagerState` account to be modified
    /// # PDA Seeds
    /// - `PORTFOLIO_TOKEN_MANAGER_STATE_SEED`
    #[account(
        mut,
        seeds = [PORTFOLIO_TOKEN_MANAGER_STATE_SEED],
        bump = portfolio_token_manager.bump,
    )]
    pub portfolio_token_manager: Account<'info, PortfolioTokenManagerState>,
}

impl<'info> PortfolioTokenManagerSetSigner<'info> {
    /// Set the attestation signer secp256k1 Ethereum address
    /// # Arguments
    /// * `attestation_signer_secp` - The new secp256k1 Ethereum address of the attestation signer (20 bytes)
    pub fn set_attestation_signer_secp(&mut self, attestation_signer_secp: [u8; 20]) -> Result<()> {
        // Update the attestation signer address
        self.portfolio_token_manager.attestation_signer_secp = attestation_signer_secp;

        Ok(())
    }
}

/// Grant a Portfolio Token Manager role to a user by initializing a `Roles` account
/// Requires `ADMIN_ROLE_PORTFOLIO_TOKEN_MANAGER` role
/// Only the `PauserRolePortfolioTokenManager` role can be added
#[derive(Accounts)]
#[instruction(role: RoleType, user: Pubkey)]
pub struct PortfolioTokenManagerGrantRole<'info> {
    /// Pays for account creation
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The account with the authority to grant Portfolio Token Manager roles
    pub authority: Signer<'info>,

    /// The `Roles` account verifying the authority has the `ADMIN_ROLE_PORTFOLIO_TOKEN_MANAGER` role
    #[account(
        seeds = [RoleType::ADMIN_ROLE_PORTFOLIO_TOKEN_MANAGER, authority.key().as_ref()],
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

impl<'info> PortfolioTokenManagerGrantRole<'info> {
    /// Add a Portfolio Token Manager role to a user
    /// # Arguments
    /// * `role` - The role to grant (must be `PauserRolePortfolioTokenManager`)
    /// * `user` - The public key of the user to grant the role to
    pub fn grant_portfolio_token_manager_role(
        &mut self,
        role: RoleType,
        user: Pubkey,
        bumps: &PortfolioTokenManagerGrantRoleBumps,
    ) -> Result<()> {
        // Only allow the PauserRolePortfolioTokenManager role to be created
        require!(
            matches!(role, RoleType::PauserRolePortfolioTokenManager),
            OndoError::InvalidRoleType
        );

        // Write to the new Roles account
        self.role_to_grant.address = user;
        self.role_to_grant.role = role;
        self.role_to_grant.bump = bumps.role_to_grant;

        emit!(RoleGranted {
            role,
            grantee: user,
            granter: self.authority.key(),
        });

        Ok(())
    }
}

/// Revoke a Portfolio Token Manager role from a user by closing their `Roles` account
/// Requires `ADMIN_ROLE_PORTFOLIO_TOKEN_MANAGER` role
/// Only the `PauserRolePortfolioTokenManager` role can be removed
#[derive(Accounts)]
pub struct PortfolioTokenManagerRevokeRole<'info> {
    /// The account with the authority to revoke Portfolio Token Manager roles
    pub authority: Signer<'info>,

    /// Receives the lamports from closing the Roles account
    #[account(mut)]
    pub recipient: SystemAccount<'info>,

    /// The `Roles` account verifying the authority has the `ADMIN_ROLE_PORTFOLIO_TOKEN_MANAGER` role
    #[account(
        seeds = [RoleType::ADMIN_ROLE_PORTFOLIO_TOKEN_MANAGER, authority.key().as_ref()],
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

impl<'info> PortfolioTokenManagerRevokeRole<'info> {
    /// Revoke a Portfolio Token Manager role from a user by closing their `Roles` account
    pub fn revoke_portfolio_token_manager_role(&mut self) -> Result<()> {
        let role_to_revoke = &self.role_to_revoke;
        // Only allow PauserRolePortfoliotokenManager roles to be revoked
        require!(
            matches!(
                role_to_revoke.role,
                RoleType::PauserRolePortfolioTokenManager
            ),
            OndoError::InvalidRoleType
        );

        emit!(RoleRevoked {
            role: role_to_revoke.role,
            grantee: role_to_revoke.address,
            revoker: self.authority.key(),
        });

        Ok(())
    }
}
