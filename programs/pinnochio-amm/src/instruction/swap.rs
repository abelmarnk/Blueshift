use constant_product_curve::{
    ConstantProduct, 
    LiquidityPair
};
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
use pinocchio_token::{instructions::Transfer, state::TokenAccount};

pub struct SwapAccounts<'a> {
    pub user: &'a AccountInfo,
    pub user_x_ata: &'a AccountInfo,
    pub user_y_ata: &'a AccountInfo,
    pub vault_x: &'a AccountInfo,
    pub vault_y: &'a AccountInfo,
    pub config: &'a AccountInfo,
    pub token_program: &'a AccountInfo,
}

impl<'a> TryFrom<&'a [AccountInfo]> for SwapAccounts<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountInfo]) -> Result<Self, Self::Error> {
        let [user, user_x_ata, user_y_ata, vault_x, vault_y, config, token_program] = accounts
        else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };

        Ok(Self { user, user_x_ata, user_y_ata, vault_x, vault_y, config, token_program })
    }
}

#[derive(Clone, Copy)]
pub struct SwapInstructionData {
    pub is_x: bool,      
    pub amount: u64,     
    pub min: u64,        
    pub expiration: i64,
}

impl<'a> TryFrom<&[u8]> for SwapInstructionData {
    type Error = ProgramError;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        if data.len() != 25 {
            return Err(ProgramError::InvalidInstructionData);
        }

        let is_x = match data[0] {
            0 => false,
            1 => true,
            _ => return Err(ProgramError::InvalidInstructionData),
        };

        // Safe because slices are exactly sized by the check above
        let amount = u64::from_le_bytes(data[1..9].try_into().unwrap());
        let min = u64::from_le_bytes(data[9..17].try_into().unwrap());
        let expiration = i64::from_le_bytes(data[17..25].try_into().unwrap());

        // Wasting gas?
        if amount == 0 || min == 0 {
            return Err(ProgramError::InvalidInstructionData);
        }

        // Expiration check
        let now = Clock::get()?.unix_timestamp;
        if expiration <= now {
            return Err(ProgramError::InvalidArgument);
        }

        Ok(Self { is_x, amount, min, expiration })
    }
}

pub struct Swap<'a> {
    pub accounts: SwapAccounts<'a>,
    pub instruction_data: SwapInstructionData,
}

impl<'a> TryFrom<(&'a [u8], &'a [AccountInfo])> for Swap<'a> {
    type Error = ProgramError;

    fn try_from((data, accounts): (&'a [u8], &'a [AccountInfo])) -> Result<Self, Self::Error> {
        let accounts = SwapAccounts::try_from(accounts)?;
        let instruction_data = SwapInstructionData::try_from(data)?;
        Ok(Self { accounts, instruction_data })
    }
}

impl<'a> Swap<'a> {
    pub const DISCRIMINATOR: &'a u8 = &3;

    // This function is only called once and unconditionally
    // It is separated for readability
    #[inline(always)]
    pub fn check(&mut self) -> Result<(u64, u64), ProgramError> {
        // Load config and guard rails
        let config = crate::state::Config::load(&self.accounts.config)?;
        if !config.can_swap() {
            return Err(ProgramError::InvalidAccountData);
        }

        // Derive vault PDAs and compare
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

        // Read vault balances
        let vault_x = unsafe { TokenAccount::from_account_info_unchecked(self.accounts.vault_x)? };
        let vault_y = unsafe { TokenAccount::from_account_info_unchecked(self.accounts.vault_y)? };

        // Initialize curve from vault reserves
        let mut curve = ConstantProduct::init(
            vault_x.amount(),
            vault_y.amount(),
            vault_x.amount(), // kept to match the original implementation
            config.fee(),
            None,
        )
        .map_err(|_| ProgramError::InvalidArgument)?;

        let pair = if self.instruction_data.is_x { LiquidityPair::X } else { LiquidityPair::Y };

        // Compute swap
        let res = curve
            .swap(pair, self.instruction_data.amount, self.instruction_data.min)
            .map_err(|_| ProgramError::InvalidArgument)?;

        if res.deposit.eq(&0) || res.withdraw.eq(&0) {
            return Err(ProgramError::InvalidArgument);
        }

        Ok((res.deposit, res.withdraw))
    }

    // This function is only called once and unconditionally
    // It is separated for readability
    #[inline(always)] 
    pub fn transfer(&mut self, deposit: u64, withdraw: u64) -> ProgramResult {
        let config = crate::state::Config::load(&self.accounts.config)?;

        // Build signer seeds for the config PDA authority
        let config_seeds = [
            Seed::from(b"config"),
            Seed::from(config.seed()),
            Seed::from(config.mint_x()),
            Seed::from(config.mint_y()),
            Seed::from(config.config_bump()),
        ];
        let signer_seeds = [Signer::from(&config_seeds)];

        match self.instruction_data.is_x {
            true => {
                // user X -> vault X
                Transfer {
                    from: self.accounts.user_x_ata,
                    to: self.accounts.vault_x,
                    authority: self.accounts.user,
                    amount: deposit,
                }
                .invoke()?;

                // vault Y -> user Y (signed by config)
                Transfer {
                    from: self.accounts.vault_y,
                    to: self.accounts.user_y_ata,
                    authority: self.accounts.config,
                    amount: withdraw,
                }
                .invoke_signed(&signer_seeds)?;
            }
            false => {
                // user Y -> vault Y
                Transfer {
                    from: self.accounts.user_y_ata,
                    to: self.accounts.vault_y,
                    authority: self.accounts.user,
                    amount: deposit,
                }
                .invoke()?;

                // vault X -> user X (signed by config)
                Transfer {
                    from: self.accounts.vault_x,
                    to: self.accounts.user_x_ata,
                    authority: self.accounts.config,
                    amount: withdraw,
                }
                .invoke_signed(&signer_seeds)?;
            }
        }

        Ok(())
    }

    pub fn process(&mut self) -> ProgramResult {
        let (deposit, withdraw) = self.check()?;
        self.transfer( deposit, withdraw)
    }
}
