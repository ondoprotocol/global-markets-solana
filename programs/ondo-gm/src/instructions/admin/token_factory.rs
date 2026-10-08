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
        NAME_AND_URI_MAX_LENGTH, PORTFOLIO_MINT_AUTHORITY_SEED, PORTFOLIO_TOKEN_MANAGER_STATE_SEED,
        SYMBOL_MAX_LENGTH,
    },
    errors::OndoError,
    events::TokenDeployed,
    state::{GMTokenManagerState, PortfolioTokenManagerState, RoleType, Roles},
    TokenKind,
};

#[derive(Copy, Clone)]
enum MintKind {
    GM,
    USDon,
    Portfolio,
}

impl MintKind {
    const fn token_kind(self) -> TokenKind {
        match self {
            MintKind::GM | MintKind::USDon => TokenKind::GM,
            MintKind::Portfolio => TokenKind::Portfolio,
        }
    }

    const fn permanent_delegate(self) -> bool {
        matches!(self, MintKind::USDon)
    }
}

/// Parameters for mint initialization
struct MintInitParams<'a, 'info> {
    payer: &'a Signer<'info>,
    mint: &'a Signer<'info>,
    mint_authority: &'a UncheckedAccount<'info>,
    system_program: &'a Program<'info, System>,
    token_program: &'a Interface<'info, TokenInterface>,
    mint_kind: MintKind,
    mint_authority_bump: u8,
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
/// * `freeze_authority` - Required nonzero freeze authority for every mint kind
/// # Returns
/// * `Result<()>` - Ok if the mint is successfully initialized, Err otherwise
fn init_mint_internal<'info>(
    params: MintInitParams<'_, 'info>,
    metadata: TokenMetadata,
    freeze_authority: &Pubkey,
) -> Result<()> {
    require_keys_neq!(
        *freeze_authority,
        Pubkey::default(),
        OndoError::InvalidMintConfiguration
    );

    let MintInitParams {
        payer,
        mint,
        mint_authority,
        system_program,
        token_program,
        mint_kind,
        mint_authority_bump,
    } = params;

    let seeds = &[mint_kind.token_kind().seed(), &[mint_authority_bump]];
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

    if mint_kind.permanent_delegate() {
        extension_types.insert(0, ExtensionType::PermanentDelegate);
    }

    let space = ExtensionType::try_calculate_account_len::<PodMint>(&extension_types)?;
    let rent = Rent::get()?;

    // Step 2: Create the mint account with enough space
    // Allocate space
    invoke(
        &system_instruction::allocate(&params.mint.key(), space as u64),
        &[mint.to_account_info()],
    )?;

    // Transfer minimum balance
    invoke(
        &system_instruction::transfer(
            &payer.key(),
            &mint.key(),
            rent.minimum_balance(space).saturating_sub(mint.lamports()),
        ),
        &[
            payer.to_account_info(),
            mint.to_account_info(),
            system_program.to_account_info(),
        ],
    )?;

    // Assign the mint account to the token program
    invoke(
        &system_instruction::assign(&params.mint.key(), &params.token_program.key()),
        &[mint.to_account_info(), system_program.to_account_info()],
    )?;

    // Step 3: Initialize extensions
    // The order matters - extensions must be initialized before the mint itself

    // Initialize ScaledUiAmount
    let init_scaled_ui_amount_ix = extension::scaled_ui_amount::instruction::initialize(
        &token_program.key(),
        &mint.key(),
        Some(mint_authority.key()),
        1f64,
    )?;
    invoke(&init_scaled_ui_amount_ix, &[mint.to_account_info()])?;

    // Initialize PermanentDelegate if needed
    if mint_kind.permanent_delegate() {
        let init_permanent_delegate_ix = initialize_permanent_delegate(
            &token_program.key(),
            &mint.key(),
            &mint_authority.key(),
        )?;
        invoke(&init_permanent_delegate_ix, &[mint.to_account_info()])?;
    }

    // Init MetadataPointer
    let init_metadata_ix = extension::metadata_pointer::instruction::initialize(
        &token_program.key(),
        &mint.key(),
        Some(mint_authority.key()),
        Some(mint.key()),
    )?;
    invoke(&init_metadata_ix, &[mint.to_account_info()])?;

    // Initialize Pausable
    let init_pausable_ix = extension::pausable::instruction::initialize(
        &token_program.key(),
        &mint.key(),
        &mint_authority.key(),
    )?;
    invoke(&init_pausable_ix, &[mint.to_account_info()])?;

    // Initialize DefaultAccountState
    let init_default_account_state_ix =
        extension::default_account_state::instruction::initialize_default_account_state(
            &token_program.key(),
            &mint.key(),
            &AccountState::Initialized,
        )?;
    invoke(&init_default_account_state_ix, &[mint.to_account_info()])?;

    // Initialize ConfidentialTransferMint
    let init_confidential_transfer_mint_ix =
        extension::confidential_transfer::instruction::initialize_mint(
            &token_program.key(),
            &mint.key(),
            Some(mint_authority.key()),
            false,
            None,
        )?;
    invoke(
        &init_confidential_transfer_mint_ix,
        &[mint.to_account_info()],
    )?;

    // Initialize TransferHook with an authority but no active hook program
    let init_transfer_hook_ix = extension::transfer_hook::instruction::initialize(
        &token_program.key(),
        &mint.key(),
        Some(mint_authority.key()),
        None,
    )?;
    invoke(&init_transfer_hook_ix, &[mint.to_account_info()])?;

    // Initialize Mint
    let init_mint_ix = initialize_mint2(
        &token_program.key(),
        &mint.key(),
        &mint_authority.key(),
        Some(freeze_authority),
        GM_TOKEN_DECIMALS,
    )?;
    invoke(&init_mint_ix, &[mint.to_account_info()])?;

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
            token_program.to_account_info(),
            TokenMetadataInitialize {
                program_id: token_program.to_account_info(),
                mint: mint.to_account_info(),
                metadata: mint.to_account_info(),
                mint_authority: mint_authority.to_account_info(),
                update_authority: mint_authority.to_account_info(),
            },
            signer_seeds,
        ),
        metadata.name,
        metadata.symbol,
        metadata.uri,
    )?;

    // Ensure account is rent-exempt
    let shortfall = rent
        .minimum_balance(mint.data_len())
        .saturating_sub(mint.lamports());

    if shortfall > 0 {
        invoke(
            &system_instruction::transfer(&payer.key(), &mint.key(), shortfall),
            &[
                payer.to_account_info(),
                mint.to_account_info(),
                system_program.to_account_info(),
            ],
        )?;
    }

    emit!(TokenDeployed {
        mint: mint.key(),
        kind: mint_kind.token_kind() as u8
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

    /// The `Roles` account verifying the user has `DEPLOYER_ROLE_TOKEN_FACTORY`
    /// # PDA Seeds
    /// - `DEPLOYER_ROLE_TOKEN_FACTORY`
    /// - The authority's address
    #[account(
        seeds = [RoleType::DEPLOYER_ROLE_TOKEN_FACTORY, authority.key().as_ref()],
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
    /// CHECK: The seeds constraint validates this authority PDA; no account data is read.
    #[account(
        seeds = [MINT_AUTHORITY_SEED],
        bump,
    )]
    pub mint_authority: UncheckedAccount<'info>,

    /// The system program
    pub system_program: Program<'info, System>,

    /// The token program (Token-2022)
    pub token_program: Interface<'info, TokenInterface>,

    /// The `GMTokenManagerState` account containing factory configuration
    /// # PDA Seeds
    /// - GMTOKEN_MANAGER_STATE_SEED
    #[account(
        seeds = [GMTOKEN_MANAGER_STATE_SEED],
        bump = gmtoken_manager_state.bump,
        constraint = !gmtoken_manager_state.factory_paused @ OndoError::TokenFactoryPaused
    )]
    pub gmtoken_manager_state: Box<Account<'info, GMTokenManagerState>>,
}

impl<'info> TokenFactory<'info> {
    const KIND: MintKind = MintKind::GM;

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
            payer: &self.payer,
            mint: &self.mint,
            mint_authority: &self.mint_authority,
            system_program: &self.system_program,
            token_program: &self.token_program,
            mint_authority_bump: bumps.mint_authority,
            mint_kind: Self::KIND,
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

    /// The `Roles` account verifying the authority has the `DEPLOYER_ROLE_TOKEN_FACTORY` role
    /// # PDA Seeds
    /// - `DEPLOYER_ROLE_TOKEN_FACTORY`
    /// - The authority's address
    #[account(
        seeds = [RoleType::DEPLOYER_ROLE_TOKEN_FACTORY, authority.key().as_ref()],
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

    /// The `GMTokenManagerState` account containing factory configuration
    /// # PDA Seeds
    /// - `GMTOKEN_MANAGER_STATE_SEED`
    #[account(
        seeds = [GMTOKEN_MANAGER_STATE_SEED],
        bump = gmtoken_manager_state.bump,
        constraint = !gmtoken_manager_state.factory_paused @ OndoError::TokenFactoryPaused
    )]
    pub gmtoken_manager_state: Box<Account<'info, GMTokenManagerState>>,
}

impl<'info> TokenFactoryDelegate<'info> {
    const KIND: MintKind = MintKind::USDon;

    /// Initialize mint WITH permanent delegate (for USDon)
    /// # Arguments
    /// * `name` - The name of the token
    /// * `symbol` - The symbol of the token
    /// * `uri` - The metadata URI for the token
    /// * `freeze_authority` - Required nonzero freeze authority for the mint
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
            payer: &self.payer,
            mint: &self.mint,
            mint_authority: &self.mint_authority,
            system_program: &self.system_program,
            token_program: &self.token_program,
            mint_authority_bump: bumps.mint_authority,
            mint_kind: Self::KIND,
        };

        let metadata = TokenMetadata { name, symbol, uri };

        init_mint_internal(params, metadata, &freeze_authority)
    }
}

/// Initialize a portfolio mint
#[derive(Accounts)]
pub struct PortfolioTokenFactory<'info> {
    /// The payer account funding the mint account creation
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The account with the authority to deploy new tokens
    pub authority: Signer<'info>,

    /// The `Roles` account verifying the authority has the `DEPLOYER_ROLE_TOKEN_FACTORY` role
    #[account(
        seeds = [RoleType::DEPLOYER_ROLE_TOKEN_FACTORY, authority.key().as_ref()],
        bump = authority_role_account.bump,
    )]
    authority_role_account: Account<'info, Roles>,

    /// The new portfolio token mint account to be initialized
    /// CHECK: Mint account - will be initialized manually with extensions
    #[account(mut)]
    pub mint: Signer<'info>,

    /// The mint authority PDA that will control the mint
    /// CHECK: This account is used as the mint authority
    #[account(
        seeds = [PORTFOLIO_MINT_AUTHORITY_SEED],
        bump,
    )]
    pub mint_authority: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,

    /// The token program (Token-2022)
    pub token_program: Interface<'info, TokenInterface>,

    /// The `PortfolioTokenManagerState` account containing factory configuration
    #[account(
        seeds = [PORTFOLIO_TOKEN_MANAGER_STATE_SEED],
        bump = portfolio_token_manager.bump,
        constraint = !portfolio_token_manager.factory_paused @ OndoError::TokenFactoryPaused
    )]
    pub portfolio_token_manager: Box<Account<'info, PortfolioTokenManagerState>>,
}

impl<'info> PortfolioTokenFactory<'info> {
    const KIND: MintKind = MintKind::Portfolio;

    /// Initialize mint WITHOUT permanent delegate (for portfolio tokens)
    /// # Arguments
    /// * `name` - The name of the token
    /// * `symbol` - The symbol of the token
    /// * `uri` - The metadata URI for the token
    /// * `freeze_authority` - The freeze authority for the mint, must be set for portfolio tokens
    /// * `bumps` - The PDA bumps for account derivation
    /// # Returns
    /// * `Result<()>` - Ok if the mint is successfully initialized, Err otherwise
    pub fn init_mint(
        &mut self,
        name: String,
        symbol: String,
        uri: String,
        freeze_authority: Pubkey,
        bumps: &PortfolioTokenFactoryBumps,
    ) -> Result<()> {
        let params = MintInitParams {
            payer: &self.payer,
            mint: &self.mint,
            mint_authority: &self.mint_authority,
            system_program: &self.system_program,
            token_program: &self.token_program,
            mint_authority_bump: bumps.mint_authority,
            mint_kind: Self::KIND,
        };

        let metadata = TokenMetadata { name, symbol, uri };

        init_mint_internal(params, metadata, &freeze_authority)
    }
}
