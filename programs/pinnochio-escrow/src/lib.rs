#![no_std]
use pinocchio::{
account_info::AccountInfo, entrypoint, nostd_panic_handler, pubkey::Pubkey, ProgramResult,
program_error::ProgramError
};

pub mod state;
pub use state::*;

pub mod instructions;
pub use instructions::*;

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
                Some((&Make::DISCRIMINATOR, other))=>{
                    Make::try_from((accounts, other))?.process()
                },
                Some((&Take::DISCRIMINATOR, _other))=>{
                    Take::try_from(accounts)?.process()
                },
                Some((&Refund::DISCRIMINATOR, _other))=>{
                    Refund::try_from(accounts)?.process()
                },
                _ =>{
                    Err(ProgramError::InvalidInstructionData)
                }
            }
}
