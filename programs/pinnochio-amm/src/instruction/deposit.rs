use constant_product_curve::ConstantProduct;
use pinocchio::{
    ProgramResult, account_info::AccountInfo, instruction::{
        Seed, 
        Signer
    }, program_error::ProgramError, 
    pubkey::{
        create_program_address
    }, sysvars::{
        Sysvar, clock::Clock
    }
};
use pinocchio_token::state::{Mint, TokenAccount};
use bytemuck::{Pod, Zeroable};

pub struct DepositAccounts<'a> {
    pub user: &'a AccountInfo,
    pub mint_lp: &'a AccountInfo,
    pub vault_x: &'a AccountInfo,
    pub vault_y: &'a AccountInfo,
    pub user_x_ata: &'a AccountInfo,
    pub user_y_ata: &'a AccountInfo,
    pub user_lp_ata: &'a AccountInfo,
    pub config: &'a AccountInfo,
    pub token_program: &'a AccountInfo,
}
 
impl<'a> TryFrom<&'a [AccountInfo]> for DepositAccounts<'a> {
  type Error = ProgramError;
 
  fn try_from(accounts: &'a [AccountInfo]) -> Result<Self, Self::Error> {
    let [user, mint_lp, vault_x, 
        vault_y, user_x_ata, user_y_ata, 
        user_lp_ata, config, token_program]  =
        accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    Ok(Self {
        user,
        mint_lp,
        vault_x,
        vault_y,
        user_x_ata,
        user_y_ata,
        user_lp_ata,
        config,
        token_program,
    })
  }
}
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct DepositInstructionData {
    pub amount: u64,
    pub max_x: u64,
    pub max_y: u64,
    pub expiration: i64,
}
 
impl<'a> TryFrom<&[u8]> for DepositInstructionData {
    type Error = ProgramError;
 
    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        if data.len() != core::mem::size_of::<Self>() {
            return Err(ProgramError::InvalidInstructionData);
        }

        let instruction_data = bytemuck::pod_read_unaligned::
            <DepositInstructionData>(data);


        // Check if values are  > 0
        if instruction_data.amount.eq(&0) || instruction_data.max_x.eq(&0) ||
            instruction_data.max_y.eq(&0) {
            return Err(ProgramError::InvalidInstructionData);
        }

        // Check if expired
        let clock = Clock::get()?;
        if clock.unix_timestamp.ge(&instruction_data.expiration) {
            return Err(ProgramError::InvalidArgument);
        }

        Ok(instruction_data)
    }
}

pub struct Deposit<'a> {
    pub accounts: DepositAccounts<'a>,
    pub instruction_data: DepositInstructionData,
}
 
impl<'a> TryFrom<(&'a [u8], &'a [AccountInfo])> for Deposit<'a> {
    type Error = ProgramError;
 
    fn try_from((data, accounts): (&'a [u8], &'a [AccountInfo])) -> Result<Self, Self::Error> {
        let accounts = DepositAccounts::try_from(accounts)?;

        let instruction_data = DepositInstructionData::try_from(data)?;
 
        Ok(Self {
            accounts,
            instruction_data,
        })
    }
}
 
impl<'a> Deposit<'a> {
    pub const DISCRIMINATOR: &'a u8 = &1;

    // This function is only called once and unconditionally
    // It is separated for readability
    #[inline(always)] 
    pub fn check(&mut self) -> Result<(u64, u64), ProgramError>{

        // Get the config account
        let config = crate::state::Config::load(&self.accounts.config)?;

        // Check if the pool state permits deposits
        if !config.can_deposit() {
            return Err(ProgramError::InvalidAccountData);
        }

        let vault_x = create_program_address(
            &[
                self.accounts.config.key(),
                self.accounts.token_program.key(),
                config.mint_x(),
                config.vault_x_bump()
            ],
            &pinocchio_associated_token_account::ID,
        ).map_err(|_| ProgramError::InvalidAccountData)?;
        
        if vault_x.ne(self.accounts.vault_x.key()) {
            return Err(ProgramError::InvalidAccountData);
        }

        let vault_y = create_program_address(
            &[
                self.accounts.config.key(),
                self.accounts.token_program.key(),
                config.mint_y(),
                config.vault_y_bump()
            ],
            &pinocchio_associated_token_account::ID,
        ).map_err(|_| ProgramError::InvalidAccountData)?;

        if vault_y.ne(self.accounts.vault_y.key()) {
            return Err(ProgramError::InvalidAccountData);
        }

        // Check mint derivation
        let mint_lp_seeds = [
            b"mint_lp".as_ref(),
            self.accounts.config.key(),
            config.mint_lp_bump()
        ];

        let mint_lp = 
            create_program_address(&mint_lp_seeds, &crate::ID).
            map_err(|_| ProgramError::InvalidAccountData)?;

        if mint_lp.ne(self.accounts.mint_lp.key()) {
            return Err(ProgramError::InvalidAccountData);
        }
        

        // Deserialize the token accounts
        let mint_lp = unsafe { 
            Mint::from_account_info_unchecked(self.accounts.mint_lp)? };
        let vault_x = unsafe { 
            TokenAccount::from_account_info_unchecked(self.accounts.vault_x)? };
        let vault_y = unsafe { 
            TokenAccount::from_account_info_unchecked(self.accounts.vault_y)? };
        
        // Grab the amounts to deposit
        let (x, y) = match mint_lp.supply().eq(&0) && 
            vault_x.amount().eq(&0) && vault_y.amount().eq(&0) {
            true => (self.instruction_data.max_x, self.instruction_data.max_y),
            false => {
                let amounts = ConstantProduct::xy_deposit_amounts_from_l(
                    vault_x.amount(),
                    vault_y.amount(),
                    mint_lp.supply(),
                    self.instruction_data.amount,
                    6,
                )
                .map_err(|_| ProgramError::InvalidArgument)?;
        
                (amounts.x, amounts.y)
            }
        };

        // Check for slippage
        if !(x.le(&self.instruction_data.max_x) && y.le(&self.instruction_data.max_y)) {
            return Err(ProgramError::InvalidArgument);
        }

        Ok((x,y))
    }

    // This function is only called once and unconditionally
    // It is separated for readability
    #[inline(always)] 
    pub fn transfer_to_vault_and_mint_to_user(&mut self, x:u64, y:u64)->ProgramResult{
        // Get the config account
        let config = crate::state::Config::load(&self.accounts.config)?;

        // The mint to instruction does not require the authority to be writable
        let config_seeds = [
            Seed::from(b"config"),
            Seed::from(config.seed()),
            Seed::from(config.mint_x()),
            Seed::from(config.mint_y()),
            Seed::from(config.config_bump())
        ];
        
        // Transfer X tokens to the vault
        
        pinocchio_token::instructions::Transfer {
            from: self.accounts.user_x_ata,
            to: self.accounts.vault_x,
            authority: self.accounts.user,
            amount: x,
        }.invoke()?;

        // Transfer Y tokens to the vault
        
        pinocchio_token::instructions::Transfer {
            from: self.accounts.user_y_ata,
            to: self.accounts.vault_y,
            authority: self.accounts.user,
            amount: y,
        }.invoke()?;

        // Mint tokens to the user
        pinocchio_token::instructions::MintTo {
            mint: self.accounts.mint_lp,
            account: self.accounts.user_lp_ata,
            mint_authority: self.accounts.config, 
            amount: self.instruction_data.amount,
        }.invoke_signed(&[Signer::from(&config_seeds)])
    }
 
    pub fn process(&mut self) -> ProgramResult {

        let (x, y) = self.check()?;

        self.transfer_to_vault_and_mint_to_user(x, y)
    }
}