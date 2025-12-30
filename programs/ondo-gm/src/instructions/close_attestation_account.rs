use anchor_lang::prelude::*;
use anchor_lang::system_program;

use crate::{
    constants::{ATTESTATION_ID_SEED, MAX_ATTESTATION_EXPIRATION},
    errors::OndoError,
    state::Attestation,
};

/// Close a single attestation account
///
/// The attestation account must be older than 30 seconds to be closed.
/// The rent from the closed account is returned to the recipient (original creator).
#[derive(Accounts)]
#[instruction(_attestation_id: [u8; 16])]
pub struct CloseAttestationAccount<'info> {
    /// The user closing the attestation account
    pub closer: Signer<'info>,

    /// The attestation account to close
    /// # PDA Seeds
    /// - ATTESTATION_ID_SEED
    /// - _attestation_id
    #[account(
        mut,
        close = recipient,
        seeds = [ATTESTATION_ID_SEED, _attestation_id.as_ref()],
        bump,
    )]
    pub attestation: Account<'info, Attestation>,

    /// The recipient of the lamports from the closed attestation account
    /// Must be the creator of the attestation
    ///
    /// CHECK: Validated against the attestation creator
    #[account(
        mut,
        address = attestation.creator
    )]
    pub recipient: UncheckedAccount<'info>,

    /// The system program
    pub system_program: Program<'info, System>,
}

impl<'info> CloseAttestationAccount<'info> {
    /// Close the attestation account if it is old enough
    /// Transfers lamports to the attestation creator
    /// # Returns
    /// * `Result<()>` - Ok if the account is successfully closed, Err otherwise
    /// # Errors
    /// * `OndoError::AttestationTooNew` - If the attestation is not old enough to close
    pub fn close_attestation_account(&mut self) -> Result<()> {
        // Validate attestation is old enough to close
        require_gt!(
            Clock::get()?.unix_timestamp,
            self.attestation.created_at + MAX_ATTESTATION_EXPIRATION,
            OndoError::AttestationTooNew
        );

        msg!("Attestation account closed: {}", self.attestation.key());

        Ok(())
    }
}

/// Batch close attestation accounts
///
/// Accounts to close are passed via remaining_accounts, constraints:
/// 1. Accounts must be marked writable
/// 2. No other accounts should present in `remaining_accounts`
/// 3. Each attestation account must be created by the recipient
/// 4. Each attestation must be older than 30 seconds
#[derive(Accounts)]
pub struct BatchCloseAttestationAccounts<'info> {
    /// The user closing the attestation accounts
    pub closer: Signer<'info>,

    /// The recipient of the lamports from closed attestation accounts
    /// Must be the creator of each attestation
    #[account(mut)]
    pub recipient: SystemAccount<'info>,

    /// The system program
    pub system_program: Program<'info, System>,
}

impl<'info> BatchCloseAttestationAccounts<'info> {
    /// Batch close attestation accounts
    /// Transfers lamports to the recipient
    /// # Arguments
    /// * `remaining_accounts` - The attestation accounts to close
    /// # Returns
    /// * `Result<()>` - Ok if all accounts are successfully closed, Err otherwise
    /// # Errors
    /// * `OndoError::ProgramMismatch` - If an account is not owned by the program
    /// * `OndoError::InvalidUser` - If the attestation creator does not match the recipient
    /// * `OndoError::AttestationTooNew` - If an attestation is not old enough to close
    pub fn batch_close_attestation_accounts(
        &mut self,
        remaining_accounts: &'info [AccountInfo<'info>],
    ) -> Result<()> {
        // Get current timestamp
        let current_timestamp = Clock::get()?.unix_timestamp;

        // Iterate over each attestation account in remaining_accounts
        for attestation_info in remaining_accounts.iter() {
            require_keys_eq!(
                *attestation_info.owner,
                crate::ID,
                OndoError::ProgramMismatch
            );

            // Deserialize attestation account
            let attestation: Account<Attestation> = Account::try_from(attestation_info)?;

            // Validate attestation creator is the recipient
            require_keys_eq!(
                attestation.creator,
                self.recipient.key(),
                OndoError::InvalidUser
            );

            // Validate attestation is old enough to close
            require_gt!(
                current_timestamp,
                attestation.created_at + MAX_ATTESTATION_EXPIRATION,
                OndoError::AttestationTooNew
            );

            // Transfer lamports to recipient
            **self.recipient.to_account_info().lamports.borrow_mut() += attestation_info.lamports();
            **attestation_info.lamports.borrow_mut() = 0;

            // Reallocate account to zero size
            attestation_info.resize(0)?;

            // Assign account to system program
            attestation_info.assign(&system_program::ID);

            msg!("Attestation account closed: {}", attestation_info.key());
        }

        Ok(())
    }
}
