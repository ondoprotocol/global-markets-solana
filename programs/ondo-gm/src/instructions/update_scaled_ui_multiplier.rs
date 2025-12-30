use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke_signed;
use anchor_spl::token_interface::{Mint, TokenInterface};

use spl_token_2022::extension::scaled_ui_amount::instruction::update_multiplier;

use crate::{
    constants::{MINT_AUTHORITY_SEED, USDON_MANAGER_STATE_SEED},
    state::{RoleType, Roles, USDonManagerState},
};

/// Update the scaled UI multiplier for a GM Token
/// Requires `UPDATE_MULTIPLIER_ROLE` role
#[derive(Accounts)]
pub struct UpdateScaledUiMultiplier<'info> {
    /// The account with the authority to update the multiplier
    #[account(mut)]
    pub authority: Signer<'info>,

    /// The `Roles` account verifying the authority has the `UPDATE_MULTIPLIER_ROLE` role
    #[account(
        seeds = [RoleType::UPDATE_MULTIPLIER_ROLE, authority.key().as_ref()],
        bump = authority_role_account.bump
    )]
    pub authority_role_account: Account<'info, Roles>,

    /// CHECK: This account is used to verify the mint authority,
    /// Does not need to be checked for correctness as it is uninitialized.
    #[account(
        seeds = [MINT_AUTHORITY_SEED],
        bump
    )]
    pub mint_authority: UncheckedAccount<'info>,

    /// The mint whose scaled UI multiplier is being updated
    #[account(
        mut,
        mint::authority = mint_authority,
        mint::token_program = token_program,
        constraint = mint.key() != usdon_manager_state.usdon_mint @ crate::errors::OndoError::InvalidInputMint
    )]
    pub mint: InterfaceAccount<'info, Mint>,

    /// The USDon manager state account, validates that the mint is not the USDon mint
    #[account(
        seeds = [USDON_MANAGER_STATE_SEED],
        bump = usdon_manager_state.bump,
    )]
    pub usdon_manager_state: Account<'info, USDonManagerState>,

    /// The token program (should be the spl_token_2022 program)
    pub token_program: Interface<'info, TokenInterface>,
}

impl<'info> UpdateScaledUiMultiplier<'info> {
    /// Update the scaled UI multiplier for the specified mint
    /// # Arguments
    /// * `new_multiplier` - The new scaled UI multiplier to set
    /// * `timestamp` - The timestamp at which the update is made
    /// * `bump` - The bump seed for the mint authority PDA
    pub fn update_scaled_ui_multiplier(
        &mut self,
        new_multiplier: f64,
        timestamp: i64,
        bump: u8,
    ) -> Result<()> {
        // Create the instruction to update the scaled UI multiplier
        let update_multiplier_ix = update_multiplier(
            &self.token_program.key(),
            &self.mint.key(),
            &self.mint_authority.key(),
            &[],
            new_multiplier,
            timestamp,
        )?;

        // Invoke the instruction with the appropriate signer seeds
        invoke_signed(
            &update_multiplier_ix,
            &[
                self.mint.to_account_info(),
                self.mint_authority.to_account_info(),
            ],
            &[&[MINT_AUTHORITY_SEED, &[bump]]],
        )?;

        Ok(())
    }
}
