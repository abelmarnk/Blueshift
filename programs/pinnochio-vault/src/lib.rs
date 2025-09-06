#![no_std]
use pinocchio::{
    account_info::AccountInfo, entrypoint, instruction::{Seed, Signer}, 
    nostd_panic_handler, program_error::ProgramError, pubkey::{find_program_address, Pubkey}, 
    ProgramResult
};
use pinocchio_system::instructions::Transfer;

nostd_panic_handler!();

entrypoint!(process_instructions);

pub const ID: Pubkey = [
    0x0f, 0x1e, 0x6b, 0x14, 0x21, 0xc0, 0x4a, 0x07,
    0x04, 0x31, 0x26, 0x5c, 0x19, 0xc5, 0xbb, 0xee,
    0x19, 0x92, 0xba, 0xe8, 0xaf, 0xd1, 0xcd, 0x07,
    0x8e, 0xf8, 0xaf, 0x70, 0x47, 0xdc, 0x11, 0xf7,
];

pub fn process_instructions(_program_id:&Pubkey, accounts:&[AccountInfo], 
        instruction_data:&[u8])->ProgramResult{
            match instruction_data.split_first(){
                Some((&Deposit::DISCRIMINATOR, other))=>{
                    Deposit::try_from((accounts, other))?.process()
                },
                Some((&Withdraw::DISCRIMINATOR, _other))=>{
                    Withdraw::try_from(accounts)?.process()
                },
                _ =>{
                    Err(ProgramError::InvalidInstructionData)
                }
            }
}

pub struct DepositAccounts<'info>{
    pub owner:&'info AccountInfo,
    pub vault:&'info AccountInfo
}

pub struct Deposit<'info>{
    accounts:DepositAccounts<'info>,
    amount:u64
}

impl<'info> TryFrom<(&'info[AccountInfo], &[u8])> for Deposit<'info>{
    fn try_from(value: (&'info[AccountInfo], &[u8])) -> Result<Self, Self::Error> {
        let accounts = value.0;
        let amount = value.1;

        let [owner, vault, _] = accounts else{
            return Err(ProgramError::InvalidArgument);
        };

        let accounts = DepositAccounts{
                                owner, 
                                vault
                            };

        let amount_bytes:[u8;8] = amount.try_into().map_err(|_| ProgramError::InvalidInstructionData)?;

        let amount = u64::from_le_bytes(amount_bytes);

        Ok(Deposit{
            accounts,
            amount
        })
    }

    type Error = ProgramError;
}

impl<'info> Deposit<'info>{
    pub const DISCRIMINATOR:u8 = 0;

    pub fn check(&self)->ProgramResult{

        if !self.accounts.owner.is_signer(){
            return Err(ProgramError::MissingRequiredSignature);
        }

        if !self.accounts.vault.is_owned_by(&pinocchio_system::ID){
            return Err(ProgramError::InvalidAccountOwner);
        }

        if self.accounts.vault.lamports().ne(&0){
            return Err(ProgramError::InvalidAccountData);
        }

        if !self.accounts.vault.data_is_empty(){
            return Err(ProgramError::InvalidAccountData);
        }

        let (expected_vault, _bump) = 
            find_program_address(&[b"vault", self.accounts.owner.key()], &ID);

        if expected_vault.ne(self.accounts.vault.key()){
            return Err(ProgramError::InvalidAccountOwner);
        }

        if self.amount.eq(&0){
            return Err(ProgramError::InvalidInstructionData);
        }

        Ok(())
    }

    pub fn process(&self)->ProgramResult{

        self.check()?;

        Transfer{
            from: self.accounts.owner,
            to: self.accounts.vault,
            lamports: self.amount
        }.invoke()
    }
}

pub struct WithdrawAccounts<'info>{
    pub owner:&'info AccountInfo,
    pub vault:&'info AccountInfo,
    pub bump:[u8;1]
}

pub struct Withdraw<'info>{
    accounts:WithdrawAccounts<'info>,
}

impl<'info> TryFrom<&'info[AccountInfo]> for Withdraw<'info>{
    fn try_from(accounts: &'info[AccountInfo]) -> Result<Self, Self::Error> {

        let [owner, vault, _] = accounts else{
            return Err(ProgramError::InvalidArgument);
        };

        let accounts = WithdrawAccounts{
                                owner, 
                                vault,
                                bump:[0] // Temporary, bump would be placed in later
                            };

        Ok(Withdraw{
            accounts
        })
    }

    type Error = ProgramError;
}

impl<'info> Withdraw<'info>{
    const DISCRIMINATOR:u8 = 1;

    pub fn check(&mut self) ->ProgramResult{

        if !self.accounts.owner.is_signer(){
            return Err(ProgramError::MissingRequiredSignature);
        }

        if !self.accounts.vault.is_owned_by(&pinocchio_system::ID){
            return Err(ProgramError::InvalidAccountOwner);
        }

        if  self.accounts.vault.lamports().eq(&0){
            return Err(ProgramError::InvalidAccountData);
        }

        if !self.accounts.vault.data_is_empty(){
            return Err(ProgramError::InvalidAccountData);
        }

        let (expected_vault, bump) = 
            find_program_address(&[b"vault", self.accounts.owner.key().as_ref()], &ID);

        self.accounts.bump[0] = bump;

        if expected_vault.ne(self.accounts.vault.key()){
            return Err(ProgramError::InvalidAccountOwner);
        }

        Ok(())
    }

    pub fn process(&mut self)->ProgramResult{

        self.check()?;

        let killing_floor = [Seed::from(b"vault"),
                Seed::from(&self.accounts.owner.key()[..]),
                Seed::from(&self.accounts.bump)
        ];

        let signer = Signer::from(&killing_floor);

        Transfer{
            from: self.accounts.vault,
            to: self.accounts.owner,
            lamports: self.accounts.vault.lamports()
        }.invoke_signed(&[signer])
    }
}



