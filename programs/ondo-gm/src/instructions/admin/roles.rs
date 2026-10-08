use anchor_lang::prelude::*;

use crate::{
    errors::OndoError,
    events::{RoleGranted, RoleRevoked},
    state::{RoleType, Roles},
};

/// Grant a role to a user by initializing a `Roles` account
/// Requires the signer to be the program upgrade authority
#[derive(Accounts)]
#[instruction(role: RoleType, user: Pubkey)]
pub struct GrantRole<'info> {
    /// Pays for account creation
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The account with the authority to grant roles, must be the program upgrade authority
    pub authority: Signer<'info>,

    /// The Roles account to be initialized
    /// # PDA Seeds
    /// - The role seed (from RoleType)
    /// - The user's address
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

    /// The Ondo Global Markets program
    #[account(address = crate::ID)]
    pub program: Program<'info, crate::program::OndoGm>,

    /// The ProgramData account of the Ondo Global Markets program
    #[account(
        constraint =
            program_data.upgrade_authority_address == Some(authority.key()) @ OndoError::InvalidUser
    )]
    pub program_data: Account<'info, ProgramData>,
}

impl<'info> GrantRole<'info> {
    /// Grant a user a role by initialize the Roles account with the specified role and bumps
    /// Validates that the signer is the program upgrade authority
    /// # Arguments
    /// * `role` - The RoleType to assign to the user
    /// * `bumps` - The bumps used for PDA derivation
    /// # Returns
    /// * `Result<()>` - Ok if successful, Err otherwise
    pub fn grant_role(
        &mut self,
        role: RoleType,
        user: Pubkey,
        bumps: &GrantRoleBumps,
    ) -> Result<()> {
        // Verify the program upgrade authority
        if let Some(program_data_address) = self.program.programdata_address()? {
            require_keys_eq!(
                program_data_address,
                self.program_data.key(),
                OndoError::ProgramMismatch
            );
        } else {
            return Err(OndoError::ProgramMismatch.into());
        }

        // Write role data to the Roles account
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

/// Revoke a role from a user by closing their `Roles` account
/// Requires the signer to be the program upgrade authority
#[derive(Accounts)]
#[instruction(_role: RoleType)]
pub struct RevokeRole<'info> {
    /// Receives funds from account closure
    #[account(mut)]
    pub recipient: SystemAccount<'info>,

    /// The account with the authority to revoke roles, must be the program upgrade authority
    pub authority: Signer<'info>,

    /// The Roles account to be closed
    /// # PDA Seeds
    /// - `role_to_revoke.role.seed()` (the seed for the role)
    /// - `role_to_revoke.address` (the user's address)
    #[account(
        mut,
        close = recipient,
        seeds = [_role.seed(), role_to_revoke.address.as_ref()],
        bump = role_to_revoke.bump
    )]
    pub role_to_revoke: Account<'info, Roles>,

    /// The system program
    pub system_program: Program<'info, System>,

    /// The Ondo Global Markets program
    #[account(address = crate::ID)]
    pub program: Program<'info, crate::program::OndoGm>,

    /// The ProgramData account of the Ondo Global Markets program
    #[account(
        constraint =
            program_data.upgrade_authority_address == Some(authority.key()) @ OndoError::InvalidUser
    )]
    pub program_data: Account<'info, ProgramData>,
}

impl<'info> RevokeRole<'info> {
    /// Revoke a role from a user by closing their Roles account
    /// Validates that the signer is the program upgrade authority
    /// # Returns
    /// * `Result<()>` - Ok if successful, Err otherwise
    pub fn revoke_role(&mut self) -> Result<()> {
        // Verify the program data address
        if let Some(program_data_address) = self.program.programdata_address()? {
            require_keys_eq!(
                program_data_address,
                self.program_data.key(),
                OndoError::ProgramMismatch
            );
        } else {
            return Err(OndoError::ProgramMismatch.into());
        }

        // Emit event for role revoked
        emit!(RoleRevoked {
            role: self.role_to_revoke.role,
            grantee: self.role_to_revoke.address,
            revoker: self.authority.key(),
        });

        Ok(())
    }
}
