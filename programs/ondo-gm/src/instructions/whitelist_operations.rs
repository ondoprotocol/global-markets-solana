use anchor_lang::prelude::*;

use crate::{
    constants::WHITELIST_SEED,
    events::{UserAddedToWhitelist, UserRemovedFromWhitelist},
    state::{RoleType, Roles, Whitelist},
};

/// Add an address to the whitelist.
/// Requires `ADMIN_ROLE_WHITELIST` role.
#[derive(Accounts)]
#[instruction(address_to_whitelist: Pubkey)]
pub struct AddToWhitelist<'info> {
    /// Pays for account creation
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The account that has the authority to add an address to the whitelist
    pub authority: Signer<'info>,

    /// The `Roles` account verifying the authority has the `ADMIN_ROLE_WHITELIST` role
    /// # PDA Seeds
    /// - ADMIN_ROLE_WHITELIST
    /// - The authority's address
    ///
    /// CHECK: Seeds constraint validates PDA address.
    #[account(
        seeds = [RoleType::ADMIN_ROLE_WHITELIST, authority.key().as_ref()],
        bump = authority_role_account.bump,
    )]
    pub authority_role_account: Account<'info, Roles>,

    /// The Whitelist account being created
    /// # PDA Seeds
    /// - WHITELIST_SEED
    /// - Address being whitelisted
    #[account(
        init,
        payer = payer,
        space = 8 + Whitelist::INIT_SPACE,
        seeds = [WHITELIST_SEED, address_to_whitelist.as_ref()],
        bump
    )]
    pub whitelist: Account<'info, Whitelist>,

    /// The system program
    pub system_program: Program<'info, System>,
}

impl<'info> AddToWhitelist<'info> {
    /// Add an address to the whitelist
    /// # Arguments
    /// * `address_to_whitelist` - The public key of the address to add to the whitelist
    /// # Returns
    /// * `Result<()>` - Ok if the address is successfully whitelisted, Err otherwise
    pub fn add_to_whitelist(&mut self, address_to_whitelist: Pubkey) -> Result<()> {
        self.whitelist.set_inner(Whitelist {
            user: address_to_whitelist,
        });

        emit!(UserAddedToWhitelist {
            user: address_to_whitelist,
            added_by: self.authority.key(),
        });

        Ok(())
    }
}

/// Remove an address from the whitelist.
/// Requires `ADMIN_ROLE_WHITELIST` role.
#[derive(Accounts)]
#[instruction(address_to_remove: Pubkey)]
pub struct RemoveFromWhitelist<'info> {
    /// The account with the authority to remove an address from the whitelist
    pub authority: Signer<'info>,

    /// Receives the lamports from closing the Whitelist account
    #[account(mut)]
    pub recipient: SystemAccount<'info>,

    /// The `Roles` account verifying the authority has the `ADMIN_ROLE_WHITELIST` role
    /// # PDA Seeds
    /// - `ADMIN_ROLE_WHITELIST`
    /// - The authority's address
    ///
    /// CHECK: Seeds constraint validates PDA address.
    #[account(
        seeds = [RoleType::ADMIN_ROLE_WHITELIST, authority.key().as_ref()],
        bump = authority_role_account.bump,
    )]
    pub authority_role_account: Account<'info, Roles>,

    /// The Whitelist account being closed
    /// # PDA Seeds
    /// - `WHITELIST_SEED`
    /// - Address being removed from whitelist
    #[account(
        mut,
        close = recipient,
        seeds = [WHITELIST_SEED, address_to_remove.as_ref()],
        bump,
    )]
    whitelist: Account<'info, Whitelist>,
}

impl<'info> RemoveFromWhitelist<'info> {
    /// Remove an address from the whitelist
    /// # Arguments
    /// * `address_to_remove` - The public key of the address to remove from the whitelist
    /// # Returns
    /// * `Result<()>` - Ok if the address is successfully removed, Err otherwise
    pub fn remove_from_whitelist(&self, address_to_remove: Pubkey) -> Result<()> {
        emit!(UserRemovedFromWhitelist {
            user: address_to_remove,
            removed_by: self.authority.key(),
        });

        Ok(())
    }
}
