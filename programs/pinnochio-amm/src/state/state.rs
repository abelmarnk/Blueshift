use core::mem::size_of;
use pinocchio::{
    account_info::{
        AccountInfo, 
        Ref, 
        RefMut
    }, 
    program_error::ProgramError, 
    pubkey::Pubkey
};
 
#[repr(C)]
pub struct Config {
    state: u8,
    seed: [u8; 8],
    authority: Pubkey,
    mint_x: Pubkey,
    mint_y: Pubkey,
    fee: [u8; 2],
    config_bump: [u8; 1],
    vault_x_bump: [u8; 1],
    vault_y_bump: [u8; 1],
    mint_lp_bump: [u8; 1],
}
 
#[repr(u8)]
pub enum AmmState {
    Initialized = 1u8,
    Disabled = 2u8,
    WithdrawOnly = 3u8,
}
 
impl Config {
    // Constants
    pub const LEN: usize = size_of::<Config>();
}

impl Config {
 
    #[inline(always)]
    pub fn load(account_info: &AccountInfo) -> Result<Ref<Self>, ProgramError> {
        if account_info.data_len() != Self::LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        if account_info.owner().ne(&crate::ID) {
            return Err(ProgramError::InvalidAccountOwner);
        }
        Ok(Ref::map(account_info.try_borrow_data()?, |data| unsafe {
            Self::from_bytes_unchecked(data)
        }))
    }
 
    #[inline(always)]
    pub unsafe fn load_unchecked(account_info: &AccountInfo) -> Result<&Self, ProgramError> {
        if account_info.data_len() != Self::LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        if account_info.owner() != &crate::ID {
            return Err(ProgramError::InvalidAccountOwner);
        }
        Ok(Self::from_bytes_unchecked(
            account_info.borrow_data_unchecked(),
        ))
    }
 
    /// Return a `Config` from the given bytes.
    ///
    /// # Safety
    ///
    /// The caller must ensure that `bytes` contains a valid representation of `Config`, and
    /// it is properly aligned to be interpreted as an instance of `Config`.
    /// At the moment `Config` has an alignment of 1 byte.
    /// This method does not perform a length validation.
    #[inline(always)]
    pub unsafe fn from_bytes_unchecked(bytes: &[u8]) -> &Self {
        &*(bytes.as_ptr() as *const Config)
    }
 
    /// Return a mutable `Config` reference from the given bytes.
    ///
    /// # Safety
    ///
    /// The caller must ensure that `bytes` contains a valid representation of `Config`.
    #[inline(always)]
    pub unsafe fn from_bytes_unchecked_mut(bytes: &mut [u8]) -> &mut Self {
        &mut *(bytes.as_mut_ptr() as *mut Config)
    }
 
    // Getter methods for safe field access
    #[inline(always)]
    pub fn state(&self) -> u8 { self.state }

    #[inline(always)]
    pub fn can_withdraw(&self) -> bool {
        self.state().eq(&(AmmState::Initialized as u8)) || self.state().eq(&(AmmState::WithdrawOnly as u8))
    }

    #[inline(always)]
    pub fn can_deposit(&self) -> bool {
        self.state().eq(&(AmmState::Initialized as u8))
    }

    #[inline(always)]
    pub fn can_swap(&self) -> bool {
        self.state().eq(&(AmmState::Initialized as u8))
    }
 
    #[inline(always)]
    pub fn seed(&self) -> &[u8;8] {&self.seed}
 
    #[inline(always)]
    pub fn authority(&self) -> &Pubkey { &self.authority }
 
    #[inline(always)]
    pub fn mint_x(&self) -> &Pubkey { &self.mint_x }
 
    #[inline(always)]
    pub fn mint_y(&self) -> &Pubkey { &self.mint_y }
 
    #[inline(always)]
    pub fn fee(&self) -> u16 { u16::from_le_bytes(self.fee) }
 
    #[inline(always)]
    pub fn config_bump(&self) -> &[u8; 1] { &self.config_bump }

    #[inline(always)]
    pub fn mint_lp_bump(&self) -> &[u8; 1] { &self.mint_lp_bump }

    #[inline(always)]
    pub fn vault_x_bump(&self) -> &[u8; 1] { &self.vault_x_bump }

    #[inline(always)]
    pub fn vault_y_bump(&self) -> &[u8; 1] { &self.vault_y_bump }
}

impl Config {
 
    #[inline(always)]
    pub fn load_mut(account_info: &AccountInfo) -> Result<RefMut<Self>, ProgramError> {
        if account_info.data_len() != Self::LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        if account_info.owner().ne(&crate::ID) {
            return Err(ProgramError::InvalidAccountOwner);
        }
        Ok(RefMut::map(account_info.try_borrow_mut_data()?, |data| unsafe {
            Self::from_bytes_unchecked_mut(data)
        }))
    }
 
    #[inline(always)]
    pub fn set_state(&mut self, state: u8) -> Result<(), ProgramError> {
        if state.ge(&(AmmState::WithdrawOnly as u8)) {
            return Err(ProgramError::InvalidAccountData);
        }
        self.state = state as u8;
        Ok(())
    }
 
    #[inline(always)]
    pub fn set_fee(&mut self, fee: u16) -> Result<(), ProgramError> {
        if fee.ge(&10_000) {
            return Err(ProgramError::InvalidAccountData);
        }
        self.fee = fee.to_le_bytes();
        Ok(())
    }


    #[inline(always)]
    pub fn set_seed(&mut self, seed: [u8;8]) {
        self.seed = seed;
    }

    #[inline(always)]
    pub fn set_authority(&mut self, authority: Pubkey) {
        self.authority = authority;
    }

    #[inline(always)]
    pub fn set_mint_x(&mut self, mint_x: Pubkey) {
        self.mint_x = mint_x;
    }

    #[inline(always)]
    pub fn set_mint_y(&mut self, mint_y: Pubkey) {
        self.mint_y = mint_y;
    }

    #[inline(always)]
    pub fn set_config_bump(&mut self, bump: [u8; 1]) {
        self.config_bump = bump;
    }

    #[inline(always)]
    pub fn set_vault_x_bump(&mut self, bump: [u8; 1]) {
        self.vault_x_bump = bump;
    }

    #[inline(always)]
    pub fn set_vault_y_bump(&mut self, bump: [u8; 1]) {
        self.vault_y_bump = bump;
    }

    #[inline(always)]
    pub fn set_mint_lp_bump(&mut self, bump: [u8; 1]) {
        self.mint_lp_bump = bump;
    }

    // ---- Updated initializer ----
    #[inline(always)]
    pub fn set_inner(
        &mut self,
        state:AmmState,
        seed: [u8;8],
        authority: Pubkey,
        mint_x: Pubkey,
        mint_y: Pubkey,
        fee: u16,
        config_bump: [u8; 1],
        vault_x_bump: [u8; 1],
        vault_y_bump: [u8; 1],
        mint_lp_bump: [u8; 1],
    ) -> Result<(), ProgramError> {
        self.set_state(state as u8)?;
        self.set_seed(seed);
        self.set_authority(authority);
        self.set_mint_x(mint_x);
        self.set_mint_y(mint_y);
        self.set_fee(fee)?;
        self.set_config_bump(config_bump);
        self.set_vault_x_bump(vault_x_bump);
        self.set_vault_y_bump(vault_y_bump);
        self.set_mint_lp_bump(mint_lp_bump);
        Ok(())
    }
 
    #[inline(always)]
    pub fn has_authority(&self) -> Option<Pubkey> {
        let bytes = self.authority();
        let chunks: &[u64; 4] = unsafe { &*(bytes.as_ptr() as *const [u64; 4]) };
        if chunks.iter().any(|&x| x != 0) {
            Some(self.authority)
        } else {
            None
        }
    }
}