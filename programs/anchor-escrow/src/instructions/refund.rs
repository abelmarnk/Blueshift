use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken, 
    token_interface::{
        Mint, 
        TokenAccount, 
        TokenInterface, 
        transfer_checked, 
        TransferChecked, 
        close_account, 
        CloseAccount
    }
};
use crate::{state::Escrow, EscrowError};

#[derive(Accounts)]
pub struct Refund<'info> {
    #[account(
        mut
    )]
    pub maker:Signer<'info>,

    #[account(
        mut,
        seeds = [b"escrow", maker.key().as_ref(), escrow.seed.to_le_bytes().as_ref()],
        bump = escrow.bump,
        has_one = maker @ EscrowError::InvalidMaker, // Necessary?
        has_one = mint_a @ EscrowError::InvalidMintA,
        close = maker
    )]
    pub escrow: Account<'info, Escrow>,

    pub mint_a:InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        associated_token::authority = escrow,
        associated_token::mint = mint_a
    )]
    pub vault: InterfaceAccount<'info, TokenAccount>,

    #[account(
        init_if_needed,
        payer = maker,
        associated_token::authority = maker,
        associated_token::mint = mint_a
    )]
    pub maker_ata_a: InterfaceAccount<'info, TokenAccount>,

    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>
}

impl<'info> Refund<'info>  {

    pub fn withdraw_and_close_vault(&mut self) ->Result<()>{

        let transfer_accounts = TransferChecked{
                authority:self.escrow.to_account_info(),
                from: self.vault.to_account_info(),
                mint: self.mint_a.to_account_info(),
                to: self.maker_ata_a.to_account_info()
            };

            let seed_bytes = self.escrow.seed.to_le_bytes();

            let bump_seed = &[self.escrow.bump];

            let signer_seeds = &[&[b"escrow", self.maker.key.as_ref(), &seed_bytes, bump_seed][..]];

            let transfer_context = CpiContext::new_with_signer(
                self.token_program.to_account_info(),
                transfer_accounts,
                signer_seeds
            );

            transfer_checked(transfer_context, self.vault.amount, self.mint_a.decimals)?;

            let close_accounts = CloseAccount{
                account:self.vault.to_account_info(),
                authority:self.escrow.to_account_info(),
                destination:self.maker.to_account_info()
            };

            let close_context = CpiContext::new_with_signer(
                self.token_program.to_account_info(), 
                close_accounts, 
                signer_seeds);

            close_account(close_context)
    }
}