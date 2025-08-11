use anchor_lang::prelude::*;
use anchor_lang::system_program;

declare_id!("22222222222222222222222222222222222222222222");

#[program]
pub mod anchor_vault {

    use super::*;

    pub fn deposit(ctx: Context<VaultAction>, amount:u64) -> Result<()> {
        
        require_eq!(ctx.accounts.vault.lamports(), 0, VaultError::VaultAlreadyExists);

        let rent = Rent::get()?;

        let minimum_balance = rent.minimum_balance(0);

        require_gt!(amount, minimum_balance, VaultError::InvalidAmount);

        let instruction = system_program::Transfer{
            from:ctx.accounts.signer.to_account_info(),
            to:ctx.accounts.vault.to_account_info(),
        };

        let context = CpiContext::
        new(ctx.accounts.system_program.to_account_info(), instruction);

        system_program::transfer(context, amount)?;

        Ok(())
    }

    pub fn withdraw(ctx: Context<VaultAction>) -> Result<()> {

        require_neq!(ctx.accounts.vault.lamports(), 0, VaultError::InvalidAmount);

        let instruction = system_program::Transfer{
            from:ctx.accounts.vault.to_account_info(),
            to:ctx.accounts.signer.to_account_info(),
        };

        let signer_seeds = &[b"vault", ctx.accounts.signer.key.as_ref(), &[ctx.bumps.vault]];

        system_program::transfer(
            CpiContext::
            new_with_signer(
                ctx.accounts.system_program.to_account_info(), instruction,
                &[&signer_seeds[..]]), 
            ctx.accounts.vault.lamports()
        )?;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct VaultAction<'info> {

    #[account(
        mut
    )]
    signer:Signer<'info>,

    #[account(
        mut,
        seeds = [b"vault", signer.key.as_ref()],
        bump
    )]
    /// CHECK: ?
    vault:UncheckedAccount<'info>,

    system_program:Program<'info, System>

}

#[error_code]
pub enum VaultError{
     #[msg("Vault already exists")]
    VaultAlreadyExists,
    #[msg("Invalid amount")]
    InvalidAmount,
}
