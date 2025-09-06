use pinocchio::{
    account_info::AccountInfo, instruction::Seed, msg, program_error::ProgramError, pubkey::create_program_address, ProgramResult
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

pub struct Take<'info>{
    accounts:TakeAccounts<'info>,
}

impl<'info> TryFrom<&'info[AccountInfo]> for Take<'info>{
    #[inline(always)]
    fn try_from(value: &'info[AccountInfo]) -> Result<Self, Self::Error> {
        let accounts = TakeAccounts::try_from(value)?;
        
        Ok(Take{
            accounts,
        })
    }

    type Error = ProgramError;
}

impl<'info> Take<'info>{
    pub const DISCRIMINATOR:u8 = 1;

    pub fn check(&self)->ProgramResult{
        // Check if the taker signed
        SignerAccount::check(self.accounts.taker)?;
        // Check if the mints are valid
        MintInterface::check(self.accounts.mint_a)?;
        MintInterface::check(self.accounts.mint_b)?;
        // Check if the ATAs is valid
        TokenAccountInterface::check(self.accounts.taker_ata_b)?;
        
        AssociatedTokenAccount::check(
            self.accounts.vault,
            self.accounts.escrow,
            self.accounts.mint_a,
            self.accounts.token_program,
            true
        )?;

        // Check that the escrow is valid and belongs to the program
        ProgramAccount::check(self.accounts.escrow, Escrow::LEN, &crate::ID)?;

        // Check that the accounts are derived correctly
        let escrow_ref = self.accounts.escrow.try_borrow_data()?;
        let escrow = Escrow::load(&escrow_ref)?;

        let escrow_pda = create_program_address( // This check also binds the maker to the escrow
            &[b"escrow", self.accounts.maker.key().as_ref(), // Though we could also check the escrow fields for the maker
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
        // Initialize the ATAs if necessary
        AssociatedTokenAccount::init_if_needed(
        self.accounts.taker_ata_a,
        self.accounts.mint_a,
        self.accounts.taker,
        self.accounts.taker,
        self.accounts.system_program,
        self.accounts.token_program,
        )?;
    
        AssociatedTokenAccount::init_if_needed(
        self.accounts.maker_ata_b,
        self.accounts.mint_b,
        self.accounts.taker,
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
        
        
        TokenAccountInterface::transfer(
            self.accounts.taker_ata_b,
            self.accounts.maker_ata_b,
            self.accounts.taker,
            escrow.receive,
            self.accounts.token_program,
            &[]
        )?;

        // Transfer the tokens from the vault to the taker's ATA
        let amount_to_recieve = 
        unsafe{
            let amount = TokenAccount::from_bytes_unchecked(
                    &self.accounts.vault.try_borrow_data()?).amount();
            amount
        };
                
                
        let seeds = [
            Seed::from(b"escrow"),
            Seed::from(escrow.maker.as_ref()),
            Seed::from(escrow.seed.as_ref()),
            Seed::from(escrow.bump.as_ref())
            ];
            
            TokenAccountInterface::transfer(
                self.accounts.vault,
                self.accounts.taker_ata_a,
                self.accounts.escrow,
                amount_to_recieve,
                self.accounts.token_program,
                &seeds
            )?;
                    
        msg!("About to close, thanks for coming to the party!");

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
    }
}

pub struct TakeAccounts<'a> {
    pub taker: &'a AccountInfo,
    pub maker: &'a AccountInfo,
    pub escrow: &'a AccountInfo,
    pub mint_a: &'a AccountInfo,
    pub mint_b: &'a AccountInfo,
    pub maker_ata_b: &'a AccountInfo,
    pub taker_ata_a: &'a AccountInfo,
    pub taker_ata_b: &'a AccountInfo,
    pub vault: &'a AccountInfo,
    pub system_program: &'a AccountInfo,
    pub token_program: &'a AccountInfo
}

impl<'a> TryFrom<&'a[AccountInfo]> for TakeAccounts<'a> {
    type Error = ProgramError;

    #[inline]
    fn try_from(accounts: &'a[AccountInfo]) -> Result<TakeAccounts<'a>, Self::Error> {
        let [taker, maker, escrow, mint_a, 
                mint_b, vault, taker_ata_a, 
                taker_ata_b, maker_ata_b, system_program, 
                token_program, _] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
        };

        Ok(TakeAccounts {
            taker,
            maker,
            escrow,
            mint_a,
            mint_b,
            taker_ata_a,
            taker_ata_b,
            maker_ata_b,
            vault,
            system_program,
            token_program
        })
    }
}
