use pinocchio::{
    ProgramResult, account_info::AccountInfo, instruction::{Seed, Signer}, program_error::ProgramError, pubkey::find_program_address, sysvars::{Sysvar, rent::Rent}
};
use pinocchio_system::{
    instructions::{
        CreateAccount
    }
};
use pinocchio_token::{
    instructions::{
        InitializeMint2
    },
    state::Mint,
    ID as TOKEN_PROGRAM_ID
};
use core::mem::{size_of, MaybeUninit};

use crate::state;


pub struct InitializeAccounts<'a> {
    pub initializer: &'a AccountInfo,
    pub mint_lp: &'a AccountInfo,
    pub config: &'a AccountInfo,
    pub token_program: &'a AccountInfo,
    pub config_bump: [u8;1],
    pub mint_lp_bump: [u8;1],
    pub vault_x_bump: [u8;1],
    pub vault_y_bump: [u8;1],
}
 
impl<'a> TryFrom<&'a [AccountInfo]> for InitializeAccounts<'a> {
  type Error = ProgramError;
 
  fn try_from(accounts: &'a [AccountInfo]) -> Result<Self, Self::Error> {
    let [initializer, mint_lp, config, _, token_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    Ok(Self {
        initializer,
        mint_lp,
        config,
        token_program,
        config_bump:[0], 
        mint_lp_bump:[0],
        vault_x_bump:[0],
        vault_y_bump:[0]

    })
  }
}



#[repr(C, packed)]
pub struct InitializeInstructionData {
    pub seed: [u8;8],
    pub fee: u16,
    pub mint_x: [u8; 32],
    pub mint_y: [u8; 32],
    pub authority: [u8; 32],
}
 
impl TryFrom<&[u8]> for InitializeInstructionData {
    type Error = ProgramError;
 
    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        const INITIALIZE_DATA_LEN_WITH_AUTHORITY: usize = size_of::<InitializeInstructionData>();
        const INITIALIZE_DATA_LEN: usize =
            INITIALIZE_DATA_LEN_WITH_AUTHORITY - size_of::<[u8; 32]>();
 
        match data.len() {
            INITIALIZE_DATA_LEN_WITH_AUTHORITY => {
                Ok(unsafe { (data.as_ptr() as *const Self).read_unaligned() })
            }
            INITIALIZE_DATA_LEN => {
                // If the authority is not present, we need to build the buffer and add it at the end before transmuting to the struct
                let mut raw: MaybeUninit<[u8; INITIALIZE_DATA_LEN_WITH_AUTHORITY]> = MaybeUninit::uninit();
                let raw_ptr = raw.as_mut_ptr() as *mut u8;
                unsafe {
                    // Copy the provided data
                    core::ptr::copy_nonoverlapping(data.as_ptr(), raw_ptr, INITIALIZE_DATA_LEN);
                    // Add the authority to the end of the buffer
                    core::ptr::write_bytes(raw_ptr.add(INITIALIZE_DATA_LEN), 0, 32);
                    // Now transmute to the struct
                    Ok((raw.as_ptr() as *const Self).read_unaligned())
                }
            }
            _ => Err(ProgramError::InvalidInstructionData),
        }
    }
}

pub struct Initialize<'a> {
    pub accounts: InitializeAccounts<'a>,
    pub instruction_data: InitializeInstructionData,
}
 
impl<'a> TryFrom<(&'a [u8], &'a [AccountInfo])> for Initialize<'a> {
    type Error = ProgramError;
 
    fn try_from((data, accounts): (&'a [u8], &'a [AccountInfo])) -> Result<Self, Self::Error> {
        let accounts = InitializeAccounts::try_from(accounts)?;
        let instruction_data: InitializeInstructionData = InitializeInstructionData::try_from(data)?;
 
        Ok(Self {
            accounts,
            instruction_data,
        })
    }
}
 
impl<'a> Initialize<'a> {

    pub const DISCRIMINATOR: &'a u8 = &0;

 
    pub fn process(&mut self) -> ProgramResult {

        // Make checks
        let mint_lp_seeds = [
            b"mint_lp",
            self.accounts.config.key().as_ref(),
        ];

        let (mint_lp, mint_lp_bump) = find_program_address(&mint_lp_seeds, &crate::ID);

        if mint_lp != *self.accounts.mint_lp.key(){
            return Err(ProgramError::InvalidInstructionData);
        }

        self.accounts.mint_lp_bump = [mint_lp_bump];

        let mint_lp_seeds = [
            Seed::from(mint_lp_seeds[0]),
            Seed::from(mint_lp_seeds[1]),
            Seed::from(&self.accounts.mint_lp_bump)
        ];
                
        let config_seeds = [
            b"config",
            self.instruction_data.seed.as_ref(),
            &self.instruction_data.mint_x.as_ref(),
            &self.instruction_data.mint_y.as_ref()
        ];

        let (config, config_bump) = 
            find_program_address(&config_seeds[..4], &crate::ID);

        self.accounts.config_bump = [config_bump];

        let config_seeds = [
            Seed::from(config_seeds[0]),
            Seed::from(config_seeds[1]),
            Seed::from(config_seeds[2]),
            Seed::from(config_seeds[3]),
            Seed::from(&self.accounts.config_bump)
        ];

        if config != *self.accounts.config.key(){
            return Err(ProgramError::InvalidInstructionData);
        }

        let (_, vault_x_bump)= find_program_address(
            &[
                self.accounts.config.key(),
                self.accounts.token_program.key(),
                self.instruction_data.mint_x.as_ref(),
            ],
            &pinocchio_associated_token_account::ID,
        );
        
        let (_, vault_y_bump) = find_program_address(
            &[
                self.accounts.config.key(),
                self.accounts.token_program.key(),
                self.instruction_data.mint_y.as_ref()
            ],
            &pinocchio_associated_token_account::ID,
        );

        // Create accouts and set data

        // Create the LP mint account
        CreateAccount {
            from: self.accounts.initializer,
            to: self.accounts.mint_lp,
            owner: &TOKEN_PROGRAM_ID,
            lamports: Rent::get()?.minimum_balance(Mint::LEN),
            space: Mint::LEN as u64,
        }.invoke_signed(&[Signer::from(&mint_lp_seeds)])?;

        // Initialize the LP mint
        InitializeMint2 {
            mint: self.accounts.mint_lp,
            decimals: 6,
            mint_authority: self.accounts.config.key(),
            freeze_authority: None
        }.invoke()?;

        // Create the config account
        CreateAccount {
            from: self.accounts.initializer,
            to: self.accounts.config,
            owner: &crate::ID,
            lamports: Rent::get()?.minimum_balance(crate::state::Config::LEN),
            space: crate::state::Config::LEN as u64
        }.invoke_signed(&[Signer::from(&config_seeds)])?;

        // Set the config data
        let mut config = 
            crate::state::Config::load_mut(self.accounts.config)?;
        
        config.set_inner(
            state::AmmState::Initialized,
            self.instruction_data.seed,
            self.instruction_data.authority,
            self.instruction_data.mint_x,
            self.instruction_data.mint_y,
            self.instruction_data.fee,
            [config_seeds[4][0]],
            [vault_x_bump],
            [vault_y_bump],        
            [mint_lp_seeds[2][0]]
        )

     }
}