use bytemuck::{
    Pod, 
    Zeroable
};
use constant_product_curve::ConstantProduct;
use pinocchio::{
    ProgramResult, 
    account_info::AccountInfo, 
    instruction::{
        Seed, 
        Signer
    }, 
    program_error::ProgramError, 
    pubkey::{
        create_program_address
    }, 
    sysvars::{
        Sysvar, 
        clock::Clock
    }
};
use pinocchio_token::state::{
    Mint, 
    TokenAccount
};

pub struct WithdrawAccounts<'a> {
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

impl<'a> TryFrom<&'a [AccountInfo]> for WithdrawAccounts<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountInfo]) -> Result<Self, Self::Error> {
        let [user, mint_lp, vault_x, vault_y, 
            user_x_ata, user_y_ata, user_lp_ata, 
            config, token_program] = accounts else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };

        Ok(Self { user, mint_lp, vault_x, vault_y, user_x_ata, 
            user_y_ata, user_lp_ata, config, token_program })
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct WithdrawInstructionData {
    pub amount: u64,
    pub min_x: u64,
    pub min_y: u64,
    pub expiration: i64,
}

impl<'a> TryFrom<&[u8]> for WithdrawInstructionData {
    type Error = ProgramError;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        if data.len() != core::mem::size_of::<Self>() {
            return Err(ProgramError::InvalidInstructionData);
        }
        let instruction_data = 
            bytemuck::pod_read_unaligned::<WithdrawInstructionData>(data);

        if instruction_data.amount.eq(&0) || instruction_data.min_x.eq(&0) 
            || instruction_data.min_y.eq(&0) {
            return Err(ProgramError::InvalidInstructionData);
        }

        let now = Clock::get()?.unix_timestamp;
        if instruction_data.expiration <= now {
            return Err(ProgramError::InvalidArgument);
        }

        Ok(instruction_data)
    }
}

pub struct Withdraw<'a> {
    pub accounts: WithdrawAccounts<'a>,
    pub instruction_data: WithdrawInstructionData,
}

impl<'a> TryFrom<(&'a [u8], &'a [AccountInfo])> for Withdraw<'a> {
    type Error = ProgramError;

    fn try_from((data, accounts): (&'a [u8], &'a [AccountInfo])) -> Result<Self, Self::Error> {
        let accounts = WithdrawAccounts::try_from(accounts)?;
        let instruction_data = WithdrawInstructionData::try_from(data)?;
        Ok(Self { accounts, instruction_data })
    }
}

impl<'a> Withdraw<'a> {
    pub const DISCRIMINATOR: &'a u8 = &2;

    // This function is only called once and unconditionally
    // It is separated for readability
    #[inline(always)] 
    pub fn check(&mut self) -> Result<(u64, u64), ProgramError> {
        let config = crate::state::Config::load(&self.accounts.config)?;

        if !config.can_withdraw() {
            return Err(ProgramError::InvalidAccountData);
        }

        // Derive vault PDAs
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

        // Derive LP mint PDA
        let mint_lp_seeds = [
            b"mint_lp".as_ref(), 
            self.accounts.config.key(),
            config.config_bump()
        ];

        let mint_lp = create_program_address(
            &mint_lp_seeds, &crate::ID
        ).map_err(|_| ProgramError::InvalidAccountData)?;


        if mint_lp.ne(self.accounts.mint_lp.key()) {
            return Err(ProgramError::InvalidAccountData);
        }

        // Deserialize accounts
        let mint_lp = unsafe { Mint::from_account_info_unchecked(self.accounts.mint_lp)? };
        let vault_x = unsafe { TokenAccount::from_account_info_unchecked(self.accounts.vault_x)? };
        let vault_y = unsafe { TokenAccount::from_account_info_unchecked(self.accounts.vault_y)? };

        // Compute withdrawal amounts
        let (x, y) = if mint_lp.supply() == self.instruction_data.amount {
            (vault_x.amount(), vault_y.amount())
        } else {
            let res = ConstantProduct::xy_withdraw_amounts_from_l(
                vault_x.amount(),
                vault_y.amount(),
                mint_lp.supply(),
                self.instruction_data.amount,
                6,
            )
            .map_err(|_| ProgramError::InvalidArgument)?;
            (res.x, res.y)
        };

        // Slippage check
        if x < self.instruction_data.min_x || y < self.instruction_data.min_y {
            return Err(ProgramError::InvalidArgument);
        }

        Ok((x, y))
    }

    // This function is only called once and unconditionally
    // It is separated for readability
    #[inline(always)] 
    pub fn transfer_tokens_and_burn_lp_tokens(&mut self, x: u64, y: u64) -> ProgramResult {
        let config = crate::state::Config::load(&self.accounts.config)?;

        let config_seeds = [
            Seed::from(b"config"),
            Seed::from(config.seed()),
            Seed::from(config.mint_x()),
            Seed::from(config.mint_y()),
            Seed::from(config.config_bump()),
        ];

        let signer_seeds = [Signer::from(&config_seeds)];

        // Transfer equivalent tokens back to user
        pinocchio_token::instructions::Transfer {
            from: self.accounts.vault_x,
            to: self.accounts.user_x_ata,
            authority: self.accounts.config,
            amount: x,
        }
        .invoke_signed(&signer_seeds)?;

        pinocchio_token::instructions::Transfer {
            from: self.accounts.vault_y,
            to: self.accounts.user_y_ata,
            authority: self.accounts.config,
            amount: y,
        }
        .invoke_signed(&signer_seeds)?;

        // Burn LP tokens from user
        pinocchio_token::instructions::Burn {
            mint: self.accounts.mint_lp,
            account: self.accounts.user_lp_ata,
            authority: self.accounts.user,
            amount: self.instruction_data.amount,
        }
        .invoke()
    }

    pub fn process(&mut self) -> ProgramResult {
        let (x, y) = self.check()?;
        self.transfer_tokens_and_burn_lp_tokens(x, y)
    }
}
