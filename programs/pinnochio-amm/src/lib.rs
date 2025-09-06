use pinocchio::{account_info::AccountInfo, entrypoint, program_error::ProgramError, pubkey::Pubkey, ProgramResult};
use pinocchio_pubkey::{
    declare_id
};

pub mod state;
pub use state::*;

pub mod instruction;
pub use instruction::*;

declare_id!("22222222222222222222222222222222222222222222");

entrypoint!(process_instruction);
 
fn process_instruction(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    match instruction_data.split_first() {
        Some((Initialize::DISCRIMINATOR, data)) => {
            Initialize::try_from((data, accounts))?.process()
        }
        Some((Deposit::DISCRIMINATOR, data)) => Deposit::try_from((data, accounts))?.process(),
        Some((Withdraw::DISCRIMINATOR, data)) => Withdraw::try_from((data, accounts))?.process(),
        Some((Swap::DISCRIMINATOR, data)) => Swap::try_from((data, accounts))?.process(),
        _ => Err(ProgramError::InvalidInstructionData),
    }
}