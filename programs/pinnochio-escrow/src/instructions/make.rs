use pinocchio::{
    account_info::AccountInfo, instruction::Seed, msg, program_error::ProgramError, pubkey::find_program_address, ProgramResult
};

use basic_helpers::{
    ProgramAccount, SignerAccount,UninitializedAccount
};
use associated_token_helpers::{
    AssociatedTokenAccount
};
use token_interface_helpers::{
    TokenAccountInterface,
    MintInterface
};

use crate::Escrow;

pub struct Make<'info>{
    accounts:MakeAccounts<'info>,
    data:MakeData
}

impl<'info> TryFrom<(&'info[AccountInfo], &[u8])> for Make<'info>{

    #[inline]
    fn try_from(value: (&'info[AccountInfo], &[u8])) -> Result<Self, Self::Error> {
        let accounts = MakeAccounts::try_from(value.0)?;
        let data = MakeData::try_from(value.1)?;
        
        Ok(Make{
            accounts,
            data
        })
    }

    type Error = ProgramError;
}

impl<'info> Make<'info>{
    pub const DISCRIMINATOR:u8 = 0;

    pub fn check(&mut self)->ProgramResult{
        // Check if the maker signed
        SignerAccount::check(self.accounts.maker)?;
        // Check if the mints are valid
        MintInterface::check(self.accounts.mint_a)?;
        MintInterface::check(self.accounts.mint_b)?;
        // Check if the maker's ATA is valid
        TokenAccountInterface::check(self.accounts.maker_ata_a)?;
        // Check that the vault and escrow are yet to exist
        UninitializedAccount::check(self.accounts.escrow)?;
        UninitializedAccount::check(self.accounts.vault)?;

        // Check that the exchange is reasonable
        if self.data.recieve.eq(&0) || self.data.amount.eq(&0){
            return Err(ProgramError::InvalidInstructionData);
        }

        // Check that the accounts are derived correctly
        let (escrow_pda, bump) = find_program_address( // Ensure the maker is bound to the escrow
            &[b"escrow", self.accounts.maker.key().as_ref(), // Not really necessary since we can check the escrow fields
            self.data.seed.as_ref()],
            &crate::ID
        );
        if self.accounts.escrow.key() != &escrow_pda {
            return Err(ProgramError::InvalidSeeds);
        }

        self.accounts.escrow_bump[0] = bump;

        AssociatedTokenAccount::check(
            self.accounts.vault,
            self.accounts.escrow,
            self.accounts.mint_a,
            self.accounts.token_program,
            false
        )
    }

    pub fn init(& mut self)->ProgramResult{
        ProgramAccount::init::<Escrow>(
            self.accounts.maker,
             self.accounts.escrow, 
            &[
            Seed::from(b"escrow"), 
            Seed::from(self.accounts.maker.key().as_ref()),
            Seed::from(self.data.seed.as_ref()),
            Seed::from(self.accounts.escrow_bump.as_ref())
            ], 
            Escrow::LEN, 
            &crate::ID)?;

        // Set the data
        let mut data_ref = self.accounts.escrow.try_borrow_mut_data()?;

        let escrow_data = Escrow::load_mut(&mut data_ref)?;

        escrow_data.set_inner(
            self.data.seed, 
            *self.accounts.maker.key(), 
            *self.accounts.mint_a.key(), 
            *self.accounts.mint_b.key(), 
            self.data.recieve, 
            self.accounts.escrow_bump
        );

        //core::mem::drop(data_ref);

        // Create the vault
        AssociatedTokenAccount::init(
            self.accounts.vault,
            self.accounts.mint_a,
            self.accounts.maker,
            self.accounts.escrow,
            self.accounts.system_program,
            self.accounts.token_program
        )
    }

    pub fn process(&mut self)->ProgramResult{

        // Perform the checks and set the bump
        self.check()?;

        // Create the accounts and set the data
        self.init()?;

        

        msg!("Ready to transfer tokens");
        
        // Transfer the tokens
        TokenAccountInterface::transfer(
            self.accounts.maker_ata_a,
            self.accounts.vault,
            self.accounts.maker,
            self.data.amount,
            self.accounts.token_program,
            &[]
        )
    }
}

pub struct MakeAccounts<'a> {
  pub maker: &'a AccountInfo,
  pub escrow: &'a AccountInfo,
  pub mint_a: &'a AccountInfo,
  pub mint_b: &'a AccountInfo,
  pub maker_ata_a: &'a AccountInfo,
  pub vault: &'a AccountInfo,
  pub system_program: &'a AccountInfo,
  pub token_program: &'a AccountInfo,
  pub escrow_bump:[u8;1]
}

impl<'a> TryFrom<&'a[AccountInfo]> for MakeAccounts<'a> {
    type Error = ProgramError;

    #[inline]
    fn try_from(accounts: &'a[AccountInfo]) -> Result<MakeAccounts<'a>, Self::Error> {
        let [maker, escrow, mint_a, mint_b,
            maker_ata_a, vault, system_program, 
            token_program, _] = accounts else{
            return Err(ProgramError::InvalidArgument);
        };

        Ok(MakeAccounts {
            maker,
            escrow,
            mint_a,
            mint_b,
            maker_ata_a,
            vault,
            system_program,
            token_program,
            escrow_bump: [0]
        })
    }
}

pub struct MakeData{
    pub amount:u64,
    pub recieve:u64,
    pub seed:[u8; 8]
}

impl TryFrom<&[u8]> for MakeData {
    type Error = ProgramError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        if value.len() != 24 {
            return Err(ProgramError::InvalidInstructionData);
        }

        let seed_bytes: [u8; 8] = value[0..8].try_into().map_err(|_| ProgramError::InvalidInstructionData)?;
        let recieve_bytes: [u8; 8] = value[8..16].try_into().map_err(|_| ProgramError::InvalidInstructionData)?;
        let amount_bytes: [u8; 8] = value[16..24].try_into().map_err(|_| ProgramError::InvalidInstructionData)?;

        let amount = u64::from_le_bytes(amount_bytes);
        let recieve = u64::from_le_bytes(recieve_bytes);

        Ok(MakeData { amount, recieve, seed:seed_bytes })
    }
}