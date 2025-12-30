use anchor_lang::{
    prelude::*,
    solana_program::{program::invoke, system_instruction},
};
use anchor_spl::token_interface::{
    token_metadata_initialize, TokenInterface, TokenMetadataInitialize,
};
use spl_token_2022::{
    extension::{self, ExtensionType},
    instruction::{initialize_mint2, initialize_permanent_delegate},
    pod::PodMint,
    state::AccountState,
};

use crate::{
    constants::{
        GMTOKEN_MANAGER_STATE_SEED, GM_TOKEN_DECIMALS, MINT_AUTHORITY_SEED,
        NAME_AND_URI_MAX_LENGTH, SYMBOL_MAX_LENGTH,
    },
    errors::OndoError,
    events::GMTokenDeployed,
    state::{GMTokenManagerState, RoleType, Roles},
};

/// Parameters for mint initialization
struct MintInitParams<'a, 'info> {
    authority: &'a Signer<'info>,
    mint: &'a Signer<'info>,
    mint_authority: &'a UncheckedAccount<'info>,
    system_program: &'a Program<'info, System>,
    token_program: &'a Interface<'info, TokenInterface>,
    gmtoken_manager_state: &'a Account<'info, GMTokenManagerState>,
    mint_authority_bump: u8,
    with_permanent_delegate: bool,
}

/// Metadata for the token
struct TokenMetadata {
    name: String,
    symbol: String,
    uri: String,
}

/// Helper function to initialize a mint with configurable extensions
/// # Arguments
/// * `params` - The mint initialization parameters containing all required accounts and configuration
/// * `metadata` - The token metadata including name, symbol, and URI
/// * `freeze_authority` - Optional freeze authority for the mint (Required if no permanent delegate)
/// # Returns
/// * `Result<()>` - Ok if the mint is successfully initialized, Err otherwise
fn init_mint_internal<'info>(
    params: MintInitParams<'_, 'info>,
    metadata: TokenMetadata,
    freeze_authority: &Pubkey,
) -> Result<()> {
    require!(
        !params.gmtoken_manager_state.factory_paused,
        OndoError::GMTokenFactoryPaused
    );

    let seeds = &[MINT_AUTHORITY_SEED, &[params.mint_authority_bump]];
    let signer_seeds = &[&seeds[..]];

    // Step 1: Calculate space needed for mint with appropriate extensions
    let mut extension_types = vec![
        ExtensionType::ScaledUiAmount,
        ExtensionType::MetadataPointer,
        ExtensionType::Pausable,
        ExtensionType::ConfidentialTransferMint,
        ExtensionType::DefaultAccountState,
        ExtensionType::TransferHook,
    ];

    if params.with_permanent_delegate {
        extension_types.insert(0, ExtensionType::PermanentDelegate);
    }

    let space = ExtensionType::try_calculate_account_len::<PodMint>(&extension_types)?;
    let rent = Rent::get()?;

    // Step 2: Create the mint account with enough space
    // Allocate space
    invoke(
        &system_instruction::allocate(&params.mint.key(), space as u64),
        &[params.mint.to_account_info()],
    )?;

    // Transfer minimum balance
    invoke(
        &system_instruction::transfer(
            &params.authority.key(),
            &params.mint.key(),
            rent.minimum_balance(space)
                .saturating_sub(params.mint.lamports()),
        ),
        &[
            params.authority.to_account_info(),
            params.mint.to_account_info(),
            params.system_program.to_account_info(),
        ],
    )?;

    // Assign the mint account to the token program
    invoke(
        &system_instruction::assign(&params.mint.key(), &params.token_program.key()),
        &[
            params.mint.to_account_info(),
            params.system_program.to_account_info(),
        ],
    )?;

    // Step 3: Initialize extensions
    // The order matters - extensions must be initialized before the mint itself

    // Initialize ScaledUiAmount
    let init_scaled_ui_amount_ix = extension::scaled_ui_amount::instruction::initialize(
        &params.token_program.key(),
        &params.mint.key(),
        Some(params.mint_authority.key()),
        1f64,
    )?;
    invoke(&init_scaled_ui_amount_ix, &[params.mint.to_account_info()])?;

    // Initialize PermanentDelegate if needed
    if params.with_permanent_delegate {
        let init_permanent_delegate_ix = initialize_permanent_delegate(
            &params.token_program.key(),
            &params.mint.key(),
            &params.mint_authority.key(),
        )?;
        invoke(
            &init_permanent_delegate_ix,
            &[params.mint.to_account_info()],
        )?;
    }

    // Init MetadataPointer
    let init_metadata_ix = extension::metadata_pointer::instruction::initialize(
        &params.token_program.key(),
        &params.mint.key(),
        Some(params.mint_authority.key()),
        Some(params.mint.key()),
    )?;
    invoke(&init_metadata_ix, &[params.mint.to_account_info()])?;

    // Initialize Pausable
    let init_pausable_ix = extension::pausable::instruction::initialize(
        &params.token_program.key(),
        &params.mint.key(),
        &params.mint_authority.key(),
    )?;
    invoke(&init_pausable_ix, &[params.mint.to_account_info()])?;

    // Initialize DefaultAccountState
    let init_default_account_state_ix =
        extension::default_account_state::instruction::initialize_default_account_state(
            &params.token_program.key(),
            &params.mint.key(),
            &AccountState::Initialized,
        )?;
    invoke(
        &init_default_account_state_ix,
        &[params.mint.to_account_info()],
    )?;

    // Initialize ConfidentialTransferMint
    let init_confidential_transfer_mint_ix =
        extension::confidential_transfer::instruction::initialize_mint(
            &params.token_program.key(),
            &params.mint.key(),
            Some(params.mint_authority.key()),
            false,
            None,
        )?;
    invoke(
        &init_confidential_transfer_mint_ix,
        &[params.mint.to_account_info()],
    )?;

    // Initialize TransferHook
    let init_transfer_hook_ix = extension::transfer_hook::instruction::initialize(
        &params.token_program.key(),
        &params.mint.key(),
        Some(params.mint_authority.key()),
        None,
    )?;
    invoke(&init_transfer_hook_ix, &[params.mint.to_account_info()])?;

    // Initialize Mint
    let init_mint_ix = initialize_mint2(
        &params.token_program.key(),
        &params.mint.key(),
        &params.mint_authority.key(),
        Some(freeze_authority),
        GM_TOKEN_DECIMALS,
    )?;
    invoke(&init_mint_ix, &[params.mint.to_account_info()])?;

    // Validate metadata field lengths
    require!(
        metadata.name.len() <= NAME_AND_URI_MAX_LENGTH
            && metadata.uri.len() <= NAME_AND_URI_MAX_LENGTH
            && metadata.symbol.len() <= SYMBOL_MAX_LENGTH,
        OndoError::MetadataFieldTooLong
    );

    // Step 4: Initialize token metadata
    token_metadata_initialize(
        CpiContext::new_with_signer(
            params.token_program.to_account_info(),
            TokenMetadataInitialize {
                program_id: params.token_program.to_account_info(),
                mint: params.mint.to_account_info(),
                metadata: params.mint.to_account_info(),
                mint_authority: params.mint_authority.to_account_info(),
                update_authority: params.mint_authority.to_account_info(),
            },
            signer_seeds,
        ),
        metadata.name,
        metadata.symbol,
        metadata.uri,
    )?;

    // Ensure account is rent-exempt
    let shortfall = rent
        .minimum_balance(params.mint.data_len())
        .saturating_sub(params.mint.lamports());

    if shortfall > 0 {
        invoke(
            &system_instruction::transfer(&params.authority.key(), &params.mint.key(), shortfall),
            &[
                params.authority.to_account_info(),
                params.mint.to_account_info(),
                params.system_program.to_account_info(),
            ],
        )?;
    }

    emit!(GMTokenDeployed {
        gm_token: params.mint.key(),
    });

    Ok(())
}

/// Initialize a mint WITHOUT permanent delegate (for GM tokens)
/// Freeze authority MUST be provided
#[derive(Accounts)]
pub struct TokenFactory<'info> {
    /// The payer account funding the mint account creation
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The account with the authority to deploy new GM tokens
    pub authority: Signer<'info>,

    /// The `Roles` account verifying the user has `DEPLOYER_ROLE_GMTOKEN_FACTORY`
    /// # PDA Seeds
    /// - `DEPLOYER_ROLE_GMTOKEN_FACTORY`
    /// - The authority's address
    #[account(
        seeds = [RoleType::DEPLOYER_ROLE_GMTOKEN_FACTORY, authority.key().as_ref()],
        bump = authority_role_account.bump,
    )]
    authority_role_account: Account<'info, Roles>,

    /// The new mint account to be initialized
    ///
    /// CHECK: Mint account - will be initialized manually with Pausable extension
    #[account(mut)]
    pub mint: Signer<'info>,

    /// The mint authority PDA that will control the mint
    /// # PDA Seeds
    /// - `MINT_AUTHORITY_SEED`
    ///
    /// CHECK: This account is used to verify the mint authority, but does not need to be checked for correctness as it is uninitialized.
    #[account(
        seeds = [MINT_AUTHORITY_SEED],
        bump,
    )]
    pub mint_authority: UncheckedAccount<'info>,

    /// The system program
    pub system_program: Program<'info, System>,

    /// The token program (Token-2022)
    pub token_program: Interface<'info, TokenInterface>,

    /// The `GmTokenManagerState` account containing factory configuration
    /// # PDA Seeds
    /// - GMTOKEN_MANAGER_STATE_SEED
    #[account(
        seeds = [GMTOKEN_MANAGER_STATE_SEED],
        bump = gmtoken_manager_state.bump,
    )]
    pub gmtoken_manager_state: Box<Account<'info, GMTokenManagerState>>,
}

impl<'info> TokenFactory<'info> {
    /// Initialize mint WITHOUT permanent delegate (for GM tokens)
    /// # Arguments
    /// * `name` - The name of the token
    /// * `symbol` - The symbol of the token
    /// * `uri` - The metadata URI for the token
    /// * `freeze_authority` - The freeze authority for the mint, must be set for GM Tokens
    /// * `bumps` - The PDA bumps for account derivation
    /// # Returns
    /// * `Result<()>` - Ok if the mint is successfully initialized, Err otherwise
    pub fn init_mint(
        &mut self,
        name: String,
        symbol: String,
        uri: String,
        freeze_authority: Pubkey,
        bumps: &TokenFactoryBumps,
    ) -> Result<()> {
        let params = MintInitParams {
            authority: &self.authority,
            mint: &self.mint,
            mint_authority: &self.mint_authority,
            system_program: &self.system_program,
            token_program: &self.token_program,
            gmtoken_manager_state: &self.gmtoken_manager_state,
            mint_authority_bump: bumps.mint_authority,
            with_permanent_delegate: false, // no permanent delegate for GM tokens
        };

        let metadata = TokenMetadata { name, symbol, uri };

        init_mint_internal(params, metadata, &freeze_authority)
    }
}

/// Initialize a mint WITH permanent delegate (for USDon)
#[derive(Accounts)]
pub struct TokenFactoryDelegate<'info> {
    /// The payer account funding the mint account creation
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The account with the authority to deploy new tokens
    pub authority: Signer<'info>,

    /// The `Roles` account verifying the authority has the `DEPLOYER_ROLE_GMTOKEN_FACTORY` role
    /// # PDA Seeds
    /// - `DEPLOYER_ROLE_GMTOKEN_FACTORY`
    /// - The authority's address
    #[account(
        seeds = [RoleType::DEPLOYER_ROLE_GMTOKEN_FACTORY, authority.key().as_ref()],
        bump = authority_role_account.bump,
    )]
    authority_role_account: Account<'info, Roles>,

    /// The new mint account to be initialized with permanent delegate
    ///
    /// CHECK: Mint account - will be initialized manually with extensions including permanent delegate
    #[account(mut)]
    pub mint: Signer<'info>,

    /// The mint authority PDA that will control the mint and act as permanent delegate
    /// # PDA Seeds
    /// - `MINT_AUTHORITY_SEED`
    ///
    /// CHECK: This account is used as the mint authority and permanent delegate
    #[account(
        seeds = [MINT_AUTHORITY_SEED],
        bump,
    )]
    pub mint_authority: UncheckedAccount<'info>,

    /// The system program
    pub system_program: Program<'info, System>,

    /// The token program (Token-2022)
    pub token_program: Interface<'info, TokenInterface>,

    /// The `GmTokenManagerState` account containing factory configuration
    /// # PDA Seeds
    /// - `GMTOKEN_MANAGER_STATE_SEED`
    #[account(
        seeds = [GMTOKEN_MANAGER_STATE_SEED],
        bump = gmtoken_manager_state.bump,
    )]
    pub gmtoken_manager_state: Box<Account<'info, GMTokenManagerState>>,
}

impl<'info> TokenFactoryDelegate<'info> {
    /// Initialize mint WITH permanent delegate (for USDon)
    /// # Arguments
    /// * `name` - The name of the token
    /// * `symbol` - The symbol of the token
    /// * `uri` - The metadata URI for the token
    /// * `bumps` - The PDA bumps for account derivation
    /// # Returns
    /// * `Result<()>` - Ok if the mint is successfully initialized, Err otherwise
    pub fn init_mint_delegate(
        &mut self,
        name: String,
        symbol: String,
        uri: String,
        freeze_authority: Pubkey,
        bumps: &TokenFactoryDelegateBumps,
    ) -> Result<()> {
        let params = MintInitParams {
            authority: &self.authority,
            mint: &self.mint,
            mint_authority: &self.mint_authority,
            system_program: &self.system_program,
            token_program: &self.token_program,
            gmtoken_manager_state: &self.gmtoken_manager_state,
            mint_authority_bump: bumps.mint_authority,
            with_permanent_delegate: true, // with permanent delegate for USDon
        };

        let metadata = TokenMetadata { name, symbol, uri };

        init_mint_internal(params, metadata, &freeze_authority)
    }
}
