use anchor_lang::prelude::*;
use anchor_spl::{
    token_2022,
    token_interface::{transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked},
};

use crate::{
    constants::{MAX_AGE_UPPER_BOUND, MINT_AUTHORITY_SEED, USDON_MANAGER_STATE_SEED},
    errors::OndoError,
    events::TokensRetrieved,
    state::{RoleType, Roles, USDonManagerState},
};

#[cfg(any(feature = "mainnet", feature = "testnet"))]
use anchor_spl::token::spl_token;

/// Initialize the USDon Manager state account
/// Requires the `GUARDIAN_USDON` role
#[derive(Accounts)]
pub struct InitializeUSDonManager<'info> {
    /// Pays for account creation
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The account with the authority to initialize the USDon Manager
    pub authority: Signer<'info>,

    /// The mint authority PDA
    /// # PDA Seeds
    /// - MINT_AUTHORITY_SEED
    ///
    /// CHECK: This account is used to verify the mint authority.
    /// Does not need to be checked for correctness as it is uninitialized.
    #[account(
        seeds = [MINT_AUTHORITY_SEED],
        bump,
    )]
    pub mint_authority: UncheckedAccount<'info>,

    /// The USDon mint account
    /// Must be initialized with mint authority as `mint_authority`
    /// and use the token-2022 program
    #[account(
        mint::authority = mint_authority,
        mint::token_program = token_2022::ID,
    )]
    pub usdon_mint: InterfaceAccount<'info, Mint>,

    /// The USDon vault token account
    /// Must be the ATA for `usdon_mint` owned by `usdon_manager_state`
    #[account(
        associated_token::mint = usdon_mint,
        associated_token::authority = usdon_manager_state,
        associated_token::token_program = token_2022::ID,
    )]
    pub usdon_vault: InterfaceAccount<'info, TokenAccount>,

    /// The USDC vault token account
    /// Must be the ATA for USDC mint owned by `usdon_manager_state`
    #[cfg(any(feature = "mainnet", feature = "testnet"))]
    #[account(
        associated_token::mint = crate::constants::USDC_MINT,
        associated_token::authority = usdon_manager_state,
        associated_token::token_program = spl_token::ID,
    )]
    pub usdc_vault: InterfaceAccount<'info, TokenAccount>,

    /// The USDC vault token account
    /// Used for non-mainnet deployments where USDC mint may differ
    #[cfg(not(any(feature = "mainnet", feature = "testnet")))]
    pub usdc_vault: InterfaceAccount<'info, TokenAccount>,

    /// The USDonManagerState account to be initialized
    /// # PDA Seeds
    /// - USDON_MANAGER_STATE_SEED
    #[account(
        init,
        payer = payer,
        space = 8 + USDonManagerState::INIT_SPACE,
        seeds = [USDON_MANAGER_STATE_SEED],
        bump
    )]
    pub usdon_manager_state: Account<'info, USDonManagerState>,

    /// The Roles account verifying the authority has the `GUARDIAN_USDON` role
    /// # PDA Seeds
    /// - GUARDIAN_USDON
    /// - The authority's address
    #[account(
        seeds = [RoleType::GUARDIAN_USDON, authority.key().as_ref()],
        bump = authority_role_account.bump,
    )]
    pub authority_role_account: Account<'info, Roles>,

    /// The system program
    pub system_program: Program<'info, System>,
}

impl<'info> InitializeUSDonManager<'info> {
    /// Initialize the USDon Manager state
    /// # Arguments
    /// * `usdon_mint` - The public key of the USDon mint
    /// * `usdon_price` - The initial USDon price (must be between MIN_USDON_PRICE and MAX_USDON_PRICE)
    /// * `oracle_price_enabled` - Whether to enable oracle price feeds
    /// * `usdc_vault` - The public key of the USDC vault
    /// * `usdon_vault` - The public key of the USDon vault
    /// * `bumps` - The PDA bumps for account derivation
    /// # Returns
    /// * `Result<()>` - Ok if the USDonManagerState is successfully initialized, Err otherwise
    #[allow(clippy::too_many_arguments)]
    pub fn initialize_usdon_manager(
        &mut self,
        oracle_price_enabled: bool,
        oracle_price_max_age: u64,
        usdc_price_update: Pubkey,
        bumps: &InitializeUSDonManagerBumps,
    ) -> Result<()> {
        // Validate oracle price max age
        require_gt!(oracle_price_max_age, 0, OndoError::InvalidOraclePriceMaxAge);

        // Validate USDC price oracle address
        require!(
            usdc_price_update != Pubkey::default(),
            OndoError::InvalidOraclePriceAddress
        );

        // Ensure oracle price max age does not exceed upper bound
        require_gte!(
            MAX_AGE_UPPER_BOUND,
            oracle_price_max_age,
            OndoError::InvalidOraclePriceMaxAge
        );

        // Write data to the USDonManagerState account
        self.usdon_manager_state.set_inner(USDonManagerState {
            owner: self.authority.key(),
            usdon_mint: self.usdon_mint.key(),
            oracle_price_enabled,
            oracle_price_max_age,
            usdc_price_update,
            usdc_vault: self.usdc_vault.key(),
            usdon_vault: self.usdon_vault.key(),
            bump: bumps.usdon_manager_state,
        });

        Ok(())
    }
}

/// Admin operations for the USDon Manager
/// Requires `ADMIN_ROLE_USDON_MANAGER` role
/// Allows enabling/disabling oracle price, setting USDC and USDon vaults, and setting USDon price
#[derive(Accounts)]
pub struct USDonManagerAdmin<'info> {
    /// The account with the authority to execute the operation
    pub authority: Signer<'info>,

    /// The USDonManagerState account to be modified
    /// # PDA Seeds
    /// - USDON_MANAGER_STATE_SEED
    #[account(
        mut,
        seeds = [USDON_MANAGER_STATE_SEED],
        bump = usdon_manager_state.bump,
    )]
    pub usdon_manager_state: Account<'info, USDonManagerState>,

    /// The Roles account verifying the authority has the `ADMIN_ROLE_USDON_MANAGER` role
    /// # PDA Seeds
    /// - ADMIN_ROLE_USDON_MANAGER
    /// - The authority's address
    #[account(
        seeds = [RoleType::ADMIN_ROLE_USDON_MANAGER, authority.key().as_ref()],
        bump = authority_role_account.bump,
    )]
    pub authority_role_account: Account<'info, Roles>,
}

impl<'info> USDonManagerAdmin<'info> {
    /// Enable or disable oracle price
    /// # Arguments
    /// * `is_enabled` - Whether oracle price should be enabled (true) or disabled (false)
    /// # Returns
    /// * `Result<()>` - Ok if the oracle price state is successfully set, Err otherwise
    pub fn enable_oracle_price(&mut self, is_enabled: bool) -> Result<()> {
        // Set the oracle price enabled state
        self.usdon_manager_state.oracle_price_enabled = is_enabled;

        Ok(())
    }

    /// Set the maximum age for oracle price data
    /// # Arguments
    /// * `oracle_price_max_age` - The new maximum age in seconds (must be > 0 and <= MAX_AGE_UPPER_BOUND)
    /// # Returns
    /// * `Result<()>` - Ok if the oracle price max age is successfully set, Err otherwise
    pub fn set_oracle_price_max_age(&mut self, oracle_price_max_age: u64) -> Result<()> {
        // Validate the new oracle price max age
        require_gt!(oracle_price_max_age, 0, OndoError::InvalidOraclePriceMaxAge);

        // Ensure it does not exceed the upper bound
        require_gte!(
            MAX_AGE_UPPER_BOUND,
            oracle_price_max_age,
            OndoError::InvalidOraclePriceMaxAge
        );

        // Set the new oracle price max age
        self.usdon_manager_state.oracle_price_max_age = oracle_price_max_age;

        Ok(())
    }

    /// Set the USDC price oracle address
    /// # Arguments
    /// * `new_price_update_address` - The new USDC price oracle public key (cannot be default/zero pubkey)
    /// # Returns
    /// * `Result<()>` - Ok if the USDC price oracle address is successfully set, Err otherwise
    pub fn set_usdc_price_update_address(
        &mut self,
        new_price_update_address: Pubkey,
    ) -> Result<()> {
        // Validate the new price update address
        require!(
            new_price_update_address != Pubkey::default(),
            OndoError::InvalidOraclePriceAddress
        );

        // Set the new USDC price update address
        self.usdon_manager_state.usdc_price_update = new_price_update_address;

        Ok(())
    }
}

/// Retrieve (withdraw) tokens from a vault
/// Requires `ADMIN_ROLE_USDON_MANAGER` role
/// This allows admins to withdraw any tokens from vaults controlled by the USDon manager
#[derive(Accounts)]
pub struct RetrieveTokens<'info> {
    /// The account with the authority to execute the retrieval operation
    pub authority: Signer<'info>,

    /// The USDonManagerState account used as authority for vault operations
    /// # PDA Seeds
    /// - USDON_MANAGER_STATE_SEED
    #[account(
        seeds = [USDON_MANAGER_STATE_SEED],
        bump = usdon_manager_state.bump,
    )]
    pub usdon_manager_state: Account<'info, USDonManagerState>,

    /// The Roles account verifying the authority has the `ADMIN_ROLE_USDON_MANAGER` role
    /// # PDA Seeds
    /// - ADMIN_ROLE_USDON_MANAGER
    /// - The authority's address
    #[account(
        seeds = [RoleType::ADMIN_ROLE_USDON_MANAGER, authority.key().as_ref()],
        bump = authority_role_account.bump,
    )]
    pub authority_role_account: Account<'info, Roles>,

    /// The mint of the token being retrieved
    #[account(
        mint::token_program = token_program,
    )]
    pub token_mint: InterfaceAccount<'info, Mint>,

    /// The source vault token account (must be owned by usdon_manager_state)
    #[account(
        mut,
        associated_token::mint = token_mint,
        associated_token::authority = usdon_manager_state,
        associated_token::token_program = token_program,
    )]
    pub source_vault: InterfaceAccount<'info, TokenAccount>,

    /// The destination token account to receive the tokens
    #[account(
        mut,
        token::mint = token_mint,
        token::token_program = token_program,
    )]
    pub destination: InterfaceAccount<'info, TokenAccount>,

    /// The token program (SPL Token or Token-2022)
    pub token_program: Interface<'info, TokenInterface>,
}

impl<'info> RetrieveTokens<'info> {
    /// Retrieve tokens from the vault
    /// # Arguments
    /// * `amount` - The amount of tokens to retrieve
    /// # Returns
    /// * `Result<()>` - Ok if the tokens are successfully retrieved, Err otherwise
    pub fn retrieve_tokens(&self, amount: u64) -> Result<()> {
        // Validate amount is not zero
        require!(amount > 0, OndoError::InvalidAmount);

        // Transfer tokens from vault to destination
        let seeds = &[USDON_MANAGER_STATE_SEED, &[self.usdon_manager_state.bump]];
        let signer_seeds = &[&seeds[..]];

        transfer_checked(
            CpiContext::new_with_signer(
                self.token_program.to_account_info(),
                TransferChecked {
                    from: self.source_vault.to_account_info(),
                    mint: self.token_mint.to_account_info(),
                    to: self.destination.to_account_info(),
                    authority: self.usdon_manager_state.to_account_info(),
                },
                signer_seeds,
            ),
            amount,
            self.token_mint.decimals,
        )?;

        // Emit event for tokens retrieved
        emit!(TokensRetrieved {
            token: self.token_mint.key(),
            to: self.destination.key(),
            amount,
            authority: self.authority.key(),
        });

        Ok(())
    }
}
