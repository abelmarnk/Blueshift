use pinocchio::{
    account_info::AccountInfo, instruction::Seed, program_error::ProgramError, 
    pubkey::{create_program_address},
    ProgramResult
};

use basic_helpers::{
    ProgramAccount, SignerAccount
};
use associated_token_helpers::{
    AssociatedTokenAccount
};
use pinocchio_token::state::TokenAccount;
use token_interface_helpers::{
    TokenAccountInterface,
    MintInterface
};

use crate::Escrow;

pub struct Refund<'info>{
    accounts:RefundAccounts<'info>,
}

impl<'info> TryFrom<&'info[AccountInfo]> for Refund<'info>{
    #[inline(always)]
    fn try_from(value: &'info[AccountInfo]) -> Result<Self, Self::Error> {
        let accounts = RefundAccounts::try_from(value)?;
        
        Ok(Refund{
            accounts,
        })
    }

    type Error = ProgramError;
}

impl<'info> Refund<'info>{
    pub const DISCRIMINATOR:u8 = 2;

    pub fn check(&self)->ProgramResult{
        // Check if the maker signed
        SignerAccount::check(self.accounts.maker)?;
        // Check if the mints are valid
        MintInterface::check(self.accounts.mint_a)?;
        // Check if the ATAs is valid
        AssociatedTokenAccount::check(
            self.accounts.vault,
            self.accounts.escrow,
            self.accounts.mint_a,
            self.accounts.token_program,
            true
        )?;
        // Check that the escrow is valid and belonsgs to the program
        ProgramAccount::check(self.accounts.escrow, Escrow::LEN, &crate::ID)?;
        // Check that the accounts are derived correctly
        let escrow_ref = self.accounts.escrow.try_borrow_data()?;
        let escrow = Escrow::load(&escrow_ref)?;

        let escrow_pda = create_program_address( // This check also binds the maker to the escrow
            &[b"escrow", self.accounts.maker.key().as_ref(), 
            escrow.seed.as_ref(), escrow.bump.as_ref()],
            &crate::ID
        )?;

        if self.accounts.escrow.key() != &escrow_pda {
            return Err(ProgramError::InvalidAccountOwner);
        }

        Ok(())
    }

    #[inline(always)]
    pub fn init(&self)->ProgramResult{
        // Initialize the maker's ATA if necessary
        AssociatedTokenAccount::init_if_needed(
        self.accounts.maker_ata_a,
        self.accounts.mint_a,
        self.accounts.maker,
        self.accounts.maker,
        self.accounts.system_program,
        self.accounts.token_program,
        )
    }
   
    pub fn process(&self)->ProgramResult{

        // Perform the checks
        self.check()?;

        // Initialize accounts if necessary
        self.init()?;

        // Transfer the tokens to the maker's ATA
        let escrow_ref = self.accounts.escrow.try_borrow_data()?;
        let escrow = Escrow::load(&escrow_ref)?;

        // Transfer the tokens from the vault to the taker's ATA
        let amount_to_recieve = 
            unsafe{
                TokenAccount::from_bytes_unchecked(
                    &self.accounts.vault.try_borrow_data()?).amount()
            };
        
        
        let seeds = [
            Seed::from(b"escrow"),
            Seed::from(escrow.maker.as_ref()),
            Seed::from(escrow.seed.as_ref()),
            Seed::from(escrow.bump.as_ref())
        ];

        TokenAccountInterface::transfer(
            self.accounts.vault,
            self.accounts.maker_ata_a,
            self.accounts.escrow,
            amount_to_recieve,
            self.accounts.token_program,
            &seeds
        )?;

        // Close the vault account
        TokenAccountInterface::close(
            self.accounts.vault, 
            self.accounts.maker, 
            self.accounts.escrow, 
            self.accounts.token_program, 
            &seeds
        )?;

        core::mem::drop(escrow_ref); // We borrow the escrow mutably in the below insruction
        
        // Close the escrow account
        ProgramAccount::close(
            self.accounts.escrow,
            self.accounts.maker
        )
    }}

pub struct RefundAccounts<'a> {
    pub maker: &'a AccountInfo,
    pub escrow: &'a AccountInfo,
    pub mint_a: &'a AccountInfo,
    pub maker_ata_a: &'a AccountInfo,
    pub vault: &'a AccountInfo,
    pub system_program: &'a AccountInfo,
    pub token_program: &'a AccountInfo
}

impl<'a> TryFrom<&'a[AccountInfo]> for RefundAccounts<'a> {
    type Error = ProgramError;

    #[inline]
    fn try_from(accounts: &'a[AccountInfo]) -> Result<RefundAccounts<'a>, Self::Error> {
        let [maker, escrow, mint_a, 
                vault, maker_ata_a, 
                system_program, token_program, _] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
        };

        Ok(RefundAccounts {
            maker,
            escrow,
            mint_a,
            maker_ata_a,
            vault,
            system_program,
            token_program
        })
    }
}
