use anchor_lang::prelude::*;

use crate::{
    constants::GMTOKEN_MANAGER_STATE_SEED,
    errors::OndoError,
    events::{RoleGranted, RoleRevoked, TokenFactoryPaused},
    state::{GMTokenManagerState, RoleType, Roles},
};

/// Grant a GM Token Factory role to a user by initializing a `Roles` account
/// Requires `ADMIN_ROLE_GMTOKEN_FACTORY` role
#[derive(Accounts)]
#[instruction(role: RoleType, user: Pubkey)]
pub struct GMTokenFactoryGrantRole<'info> {
    /// Pays for account creation
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The account with the authority to grant a GM Token Factory role
    pub authority: Signer<'info>,

    /// The `Roles` account verifying the authority has the `ADMIN_ROLE_GMTOKEN_FACTORY` role
    /// # PDA Seeds
    /// - `ADMIN_ROLE_GMTOKEN_FACTORY`
    /// - The authority's address
    #[account(
        seeds = [RoleType::ADMIN_ROLE_GMTOKEN_FACTORY, authority.key().as_ref()],
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

impl<'info> GMTokenFactoryGrantRole<'info> {
    /// Grant a GM Token Factory role to a user by initializing a `Roles` account
    /// # Arguments
    /// * `role` - The role to grant (must be `PauserRoleGmtokenFactory` or `DeployerRoleGmtokenFactory`)
    /// * `user` - The public key of the user to grant the role to
    /// * `bumps` - The PDA bumps for account derivation
    /// # Returns
    /// * `Result<()>` - Ok if the role is successfully granted, Err otherwise
    pub fn grant_gmtoken_factory_role(
        &mut self,
        role: RoleType,
        user: Pubkey,
        bumps: &GMTokenFactoryGrantRoleBumps,
    ) -> Result<()> {
        // Only allow `PauserRoleGmtokenFactory` and `DeployerRoleGmtokenFactory` roles to be created
        require!(
            matches!(
                role,
                RoleType::PauserRoleGMTokenFactory | RoleType::DeployerRoleGMTokenFactory
            ),
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

/// Revoke a GM Token Factory role from a user by closing their `Roles` account
/// Requires `ADMIN_ROLE_GMTOKEN_FACTORY` role
#[derive(Accounts)]
pub struct GMTokenFactoryRevokeRole<'info> {
    /// The account with the authority to revoke a GM Token Factory role
    pub authority: Signer<'info>,

    /// Receives the lamports from closing the account
    #[account(mut)]
    pub recipient: SystemAccount<'info>,

    /// The `Roles` account verifying the authority has the `ADMIN_ROLE_GMTOKEN_FACTORY` role
    /// # PDA Seeds
    /// - `ADMIN_ROLE_GMTOKEN_FACTORY`
    /// - The authority's address
    #[account(
        seeds = [RoleType::ADMIN_ROLE_GMTOKEN_FACTORY, authority.key().as_ref()],
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

impl<'info> GMTokenFactoryRevokeRole<'info> {
    /// Revoke a GM Token Factory role from a user by closing their `Roles` account
    /// # Returns
    /// * `Result<()>` - Ok if the role is successfully revoked, Err otherwise
    pub fn revoke_gmtoken_factory_role(&mut self) -> Result<()> {
        // Only allow PauserRoleGmtokenFactory and DeployerRoleGmtokenFactory roles to be revoked
        require!(
            matches!(
                self.role_to_revoke.role,
                RoleType::PauserRoleGMTokenFactory | RoleType::DeployerRoleGMTokenFactory
            ),
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

// GM Token Factory Pauser Operations
#[derive(Accounts)]
pub struct GMTokenFactoryPauser<'info> {
    /// The account with the authority to execute the pause operation
    pub authority: Signer<'info>,

    /// The `Roles` account verifying the authority has the `PAUSER_ROLE_GMTOKEN_FACTORY` role
    /// # PDA Seeds
    /// - `PAUSER_ROLE_GMTOKEN_FACTORY`
    /// - The authority's address
    #[account(
        seeds = [RoleType::PAUSER_ROLE_GMTOKEN_FACTORY, authority.key().as_ref()],
        bump = authority_role_account.bump,
    )]
    pub authority_role_account: Account<'info, Roles>,

    /// The `GmTokenManagerState` account to be modified
    /// # PDA Seeds
    /// - `GMTOKEN_MANAGER_STATE_SEED`
    #[account(
        mut,
        seeds = [GMTOKEN_MANAGER_STATE_SEED],
        bump = gmtoken_manager_state.bump,
    )]
    pub gmtoken_manager_state: Account<'info, GMTokenManagerState>,
}

impl<'info> GMTokenFactoryPauser<'info> {
    /// Pause the GM Token Factory
    /// # Returns
    /// * `Result<()>` - Ok if the factory is successfully paused, Err otherwise
    pub fn pause_factory(&mut self) -> Result<()> {
        // Set the factory_paused flag to true
        self.gmtoken_manager_state.factory_paused = true;

        // Emit event for factory paused
        emit!(TokenFactoryPaused {
            is_paused: true,
            pauser: self.authority.key(),
        });

        Ok(())
    }
}

// GM Token Factory Admin Operations
#[derive(Accounts)]
pub struct GMTokenFactoryAdmin<'info> {
    /// The account with the authority to execute the pause/unpause operation
    pub authority: Signer<'info>,

    /// The `Roles` account verifying the authority has `ADMIN_ROLE_GMTOKEN_FACTORY` role
    /// # PDA Seeds
    /// - `ADMIN_ROLE_GMTOKEN_FACTORY`
    /// - The authority's address
    #[account(
        seeds = [RoleType::ADMIN_ROLE_GMTOKEN_FACTORY, authority.key().as_ref()],
        bump = authority_role_account.bump,
    )]
    pub authority_role_account: Account<'info, Roles>,

    /// The `GmTokenManagerState` account to be modified
    /// # PDA Seeds
    /// - `GMTOKEN_MANAGER_STATE_SEED`
    #[account(
        mut,
        seeds = [GMTOKEN_MANAGER_STATE_SEED],
        bump = gmtoken_manager_state.bump,
    )]
    pub gmtoken_manager_state: Account<'info, GMTokenManagerState>,
}

impl<'info> GMTokenFactoryAdmin<'info> {
    /// Resume the GM Token Factory
    /// # Returns
    /// * `Result<()>` - Ok if the factory is successfully resumed, Err otherwise
    pub fn resume_factory(&mut self) -> Result<()> {
        // Set the factory_paused flag to false
        self.gmtoken_manager_state.factory_paused = false;

        // Emit event for factory pause state change
        emit!(TokenFactoryPaused {
            is_paused: false,
            pauser: self.authority.key(),
        });

        Ok(())
    }

    /// Pause the GM Token Factory
    /// # Returns
    /// * `Result<()>` - Ok if the factory is successfully paused, Err otherwise
    pub fn pause_factory(&mut self) -> Result<()> {
        // Set the factory_paused flag to true
        self.gmtoken_manager_state.factory_paused = true;

        // Emit event for factory pause state change
        emit!(TokenFactoryPaused {
            is_paused: true,
            pauser: self.authority.key(),
        });

        Ok(())
    }
}
