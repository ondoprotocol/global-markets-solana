use anchor_lang::prelude::*;
use anchor_lang::solana_program::{program::invoke, system_instruction};

use anchor_spl::{
    token_2022_extensions::spl_token_metadata_interface::state::Field,
    token_2022_extensions::{token_metadata_update_field, TokenMetadataUpdateField},
    token_interface::{Mint, TokenInterface},
};

use crate::{
    constants::{
        MINT_AUTHORITY_SEED, NAME_AND_URI_MAX_LENGTH, SYMBOL_MAX_LENGTH, USDON_MANAGER_STATE_SEED,
    },
    errors::OndoError,
    state::{RoleType, Roles, USDonManagerState},
};

/// Update the metadata of a Token
/// Requires `UPDATE_METADATA_ROLE` role
#[derive(Accounts)]
pub struct UpdateTokenMetadata<'info> {
    /// Pays for fees if needed
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The operator updating the metadata, if multisig then this is different to the payer
    /// Otherwise, the operator is the same as the payer
    pub authority: Signer<'info>,

    /// The `Roles` account verifying the authority has `UPDATE_METADATA_ROLE` role
    #[account(
        seeds = [RoleType::UPDATE_METADATA_ROLE, authority.key().as_ref()],
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

    /// The mint whose metadata is being updated
    #[account(
        mut,
        mint::authority = mint_authority,
        mint::token_program = token_program,
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

    /// The system program
    pub system_program: Program<'info, System>,
}

impl<'info> UpdateTokenMetadata<'info> {
    /// Update the token metadata for the specified mint
    /// # Arguments
    /// * `new_name` - The new name to set (if any)
    /// * `new_symbol` - The new symbol to set (if any)
    /// * `new_uri` - The new URI to set (if any)
    /// * `bumps` - The bumps used for PDA derivation
    /// # Returns
    /// * `Result<()>` - Ok if successful, Err otherwise
    pub fn update_token_metadata(
        &mut self,
        new_name: Option<String>,
        new_symbol: Option<String>,
        new_uri: Option<String>,
        bumps: UpdateTokenMetadataBumps,
    ) -> Result<()> {
        require!(
            new_name.is_some() || new_symbol.is_some() || new_uri.is_some(),
            OndoError::NoMetadataFieldsToUpdate
        );

        if let Some(name) = new_name {
            require_gte!(
                NAME_AND_URI_MAX_LENGTH,
                name.len(),
                OndoError::MetadataFieldTooLong
            );
            self.update_token_metadata_internal(Field::Name, name, bumps.mint_authority)?;
        }

        if let Some(symbol) = new_symbol {
            require_gte!(
                SYMBOL_MAX_LENGTH,
                symbol.len(),
                OndoError::MetadataFieldTooLong
            );
            self.update_token_metadata_internal(Field::Symbol, symbol, bumps.mint_authority)?;
        }

        if let Some(uri) = new_uri {
            require_gte!(
                NAME_AND_URI_MAX_LENGTH,
                uri.len(),
                OndoError::MetadataFieldTooLong
            );
            self.update_token_metadata_internal(Field::Uri, uri, bumps.mint_authority)?;
        }

        let mint_info = self.mint.to_account_info();

        let shortfall = Rent::get()?
            .minimum_balance(mint_info.data_len())
            .saturating_sub(mint_info.lamports());

        if shortfall > 0 {
            invoke(
                &system_instruction::transfer(&self.payer.key(), &self.mint.key(), shortfall),
                &[
                    self.payer.to_account_info(),
                    mint_info,
                    self.system_program.to_account_info(),
                ],
            )?;
        }

        Ok(())
    }

    /// Helper function to update a specific field in the token metadata that executes the CPI
    /// # Arguments
    /// * `field` - The field to update
    /// * `value` - The new value to set
    /// * `bump` - The bump seed for the mint authority PDA
    /// # Returns
    /// * `Result<()>` - Ok if successful, Err otherwise
    #[inline(always)]
    fn update_token_metadata_internal(
        &mut self,
        field: Field,
        value: String,
        bump: u8,
    ) -> Result<()> {
        token_metadata_update_field(
            CpiContext::new_with_signer(
                self.token_program.to_account_info(),
                TokenMetadataUpdateField {
                    program_id: self.token_program.to_account_info(),
                    metadata: self.mint.to_account_info(),
                    update_authority: self.mint_authority.to_account_info(),
                },
                &[&[MINT_AUTHORITY_SEED, &[bump]]],
            ),
            field,
            value,
        )
    }
}
