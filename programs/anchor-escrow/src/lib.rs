use anchor_lang::prelude::*;

declare_id!("22222222222222222222222222222222222222222222");

pub mod state;
pub use state::*;

pub mod errors;
pub use errors::*; 

pub mod instructions;
pub use instructions::*;

#[program]
pub mod anchor_escrow {
    use super::*;
    pub fn make(ctx: Context<Make>, seed:u64, amount_deposited:u64, amount_expected:u64) -> Result<()> {
        require_gt!(amount_deposited, 0, EscrowError::InvalidAmount);
        require_gt!(amount_expected, 0, EscrowError::InvalidAmount);

        ctx.accounts.populate_escrow(seed, amount_expected, ctx.bumps.escrow);

        ctx.accounts.transfer_tokens(amount_deposited)
    }

    pub fn take(ctx: Context<Take>) -> Result<()> {
        ctx.accounts.transfer_tokens()?;

        ctx.accounts.withdraw_and_close_vault()
    }
    pub fn refund(ctx: Context<Refund>) -> Result<()> {
        ctx.accounts.withdraw_and_close_vault()
    }
}



