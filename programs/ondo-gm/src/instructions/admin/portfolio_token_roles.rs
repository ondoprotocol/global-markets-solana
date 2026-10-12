use anchor_lang::prelude::*;

use crate::{
    errors::OndoError,
    events::{RoleGranted, RoleRevoked},
    state::{RoleType, Roles},
};

/// Grant a Portfolio Token role to a user by initializing a `Roles` account
/// Requires `ADMIN_ROLE_PORTFOLIO_TOKEN` role
#[derive(Accounts)]
#[instruction(role: RoleType, user: Pubkey)]
pub struct PortfolioTokenGrantRole<'info> {
    /// Pays for account creation
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The account with the authority to grant portfolio token roles
    pub authority: Signer<'info>,

    /// The `Roles` account verifying the authority has the `ADMIN_ROLE_PORTFOLIO_TOKEN` role
    #[account(
        seeds = [RoleType::ADMIN_ROLE_PORTFOLIO_TOKEN, authority.key().as_ref()],
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

impl<'info> PortfolioTokenGrantRole<'info> {
    /// Grant a Portfolio Token role to a user
    /// # Arguments
    /// * `role` - The role to grant, must be one of the following:
    ///     - `MinterRolePortfolioToken`
    ///     - `PauserRolePortfolioToken`,
    ///     - `UnpauserRolePortfolioToken`
    /// * `user` - The public key of the user to grant the role to
    pub fn grant_portfolio_token_role(
        &mut self,
        role: RoleType,
        user: Pubkey,
        bumps: &PortfolioTokenGrantRoleBumps,
    ) -> Result<()> {
        require!(
            matches!(
                role,
                RoleType::MinterRolePortfolioToken
                    | RoleType::PauserRolePortfolioToken
                    | RoleType::UnpauserRolePortfolioToken
            ),
            OndoError::InvalidRoleType
        );

        // Write to the Roles account
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

/// Revoke a Portfolio Token role from a user by closing their `Roles` account
/// Requires `ADMIN_ROLE_PORTFOLIO_TOKEN` role
#[derive(Accounts)]
pub struct PortfolioTokenRevokeRole<'info> {
    /// The recipient of the closed account lamports
    #[account(mut)]
    pub recipient: SystemAccount<'info>,

    /// The account with the authority to revoke portfolio token roles
    pub authority: Signer<'info>,

    /// The `Roles` account verifying the authority has the `ADMIN_ROLE_PORTFOLIO_TOKEN` role
    #[account(
        seeds = [RoleType::ADMIN_ROLE_PORTFOLIO_TOKEN, authority.key().as_ref()],
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

impl<'info> PortfolioTokenRevokeRole<'info> {
    /// Revoke a Portfolio Token role from a user by closing their `Roles` account
    pub fn revoke_portfolio_token_role(&mut self) -> Result<()> {
        require!(
            matches!(
                self.role_to_revoke.role,
                RoleType::MinterRolePortfolioToken
                    | RoleType::PauserRolePortfolioToken
                    | RoleType::UnpauserRolePortfolioToken
            ),
            OndoError::InvalidRoleType
        );

        emit!(RoleRevoked {
            role: self.role_to_revoke.role,
            grantee: self.role_to_revoke.address,
            revoker: self.authority.key(),
        });

        Ok(())
    }
}
