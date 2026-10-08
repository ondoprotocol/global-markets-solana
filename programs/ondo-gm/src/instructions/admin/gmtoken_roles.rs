use anchor_lang::prelude::*;

use crate::{
    errors::OndoError,
    events::{RoleGranted, RoleRevoked},
    state::{RoleType, Roles},
};

/// Grant a GM minter, mint pauser, or mint unpauser role using `AdminRoleGMToken`.
/// Creates a program-wide role PDA and emits `RoleGranted`.
/// Update-multiplier and metadata roles require the program upgrade authority.
#[derive(Accounts)]
#[instruction(role: RoleType, user: Pubkey)]
pub struct GMTokenGrantRole<'info> {
    /// Pays for account creation
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The account with the authority to grant GM Token roles
    pub authority: Signer<'info>,

    /// The `Roles` account verifying the authority has the `ADMIN_ROLE_GMTOKEN` role
    /// # PDA Seeds
    /// - ADMIN_ROLE_GMTOKEN
    /// - The authority's address
    #[account(
        seeds = [RoleType::ADMIN_ROLE_GMTOKEN, authority.key().as_ref()],
        bump = authority_role_account.bump,
    )]
    pub authority_role_account: Account<'info, Roles>,

    /// The new Roles account being created for the user
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

impl<'info> GMTokenGrantRole<'info> {
    /// Grant a GM Token role to a user
    /// # Arguments
    /// * `role` - The role to grant (must be `MinterRoleGMToken`, `PauserRoleGMToken`, or `UnpauserRoleGMToken`)
    /// * `user` - The public key of the user to grant the role to
    /// * `bumps` - The PDA bumps for account derivation
    /// # Returns
    /// * `Result<()>` - Ok if the role is successfully granted, Err otherwise
    pub fn grant_gmtoken_role(
        &mut self,
        role: RoleType,
        user: Pubkey,
        bumps: &GMTokenGrantRoleBumps,
    ) -> Result<()> {
        // Validate that the role being added is `MinterRoleGMToken`, `PauserRoleGMToken`, or `UnpauserRoleGMToken`
        require!(
            matches!(
                role,
                RoleType::MinterRoleGMToken
                    | RoleType::PauserRoleGMToken
                    | RoleType::UnpauserRoleGMToken
            ),
            OndoError::InvalidRoleType
        );

        // Write to the Roles account
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

/// Revoke a GM minter, mint pauser, or mint unpauser role using `AdminRoleGMToken`.
/// Closes the role PDA, refunds its rent to `recipient`, and emits `RoleRevoked`.
#[derive(Accounts)]
pub struct GMTokenRevokeRole<'info> {
    /// The recipient of the closed account lamports
    #[account(mut)]
    pub recipient: SystemAccount<'info>,

    /// The account with the authority to revoke GM Token roles
    pub authority: Signer<'info>,

    /// The `Roles` account verifying the authority has the `ADMIN_ROLE_GMTOKEN` role
    /// # PDA Seeds
    /// - ADMIN_ROLE_GMTOKEN
    /// - The authority's address
    #[account(
        seeds = [RoleType::ADMIN_ROLE_GMTOKEN, authority.key().as_ref()],
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
}

impl<'info> GMTokenRevokeRole<'info> {
    /// Revoke a GM Token role from a user by closing their `Roles` account
    /// # Returns
    /// * `Result<()>` - Ok if the role is successfully revoked, Err otherwise
    pub fn revoke_gmtoken_role(&mut self) -> Result<()> {
        // Validate that the role being removed is `MinterRoleGMToken`, `PauserRoleGMToken`, or `UnpauserRoleGMToken`
        require!(
            matches!(
                self.role_to_revoke.role,
                RoleType::MinterRoleGMToken
                    | RoleType::PauserRoleGMToken
                    | RoleType::UnpauserRoleGMToken
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
