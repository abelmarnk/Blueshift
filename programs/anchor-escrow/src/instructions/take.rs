use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken, 
    token_interface::{
        Mint, 
        TokenAccount, 
        TokenInterface, 
        close_account, 
        transfer_checked, 
        CloseAccount, 
        TransferChecked
    }
};
use crate::{state::Escrow, EscrowError};

#[derive(Accounts)]
pub struct Take<'info> {

    #[account(
        mut
    )]
    pub taker:Signer<'info>,

    #[account(
        mut
    )]
    /// CHECK: This account is checked with the has_one constraint 
    pub maker:UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [b"escrow", maker.key().as_ref(), escrow.seed.to_le_bytes().as_ref()],
        bump = escrow.bump,
        has_one = maker @ EscrowError::InvalidMaker, // This check is not necessary since the escrow is derived from the maker
        has_one = mint_a @ EscrowError::InvalidMintA,
        has_one = mint_b @ EscrowError::InvalidMintB,
        close = maker
    )]
    pub escrow: Box<Account<'info, Escrow>>,

    pub mint_a:Box<InterfaceAccount<'info, Mint>>,

    pub mint_b:Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        associated_token::authority = escrow,
        associated_token::mint = mint_a
    )]
    pub vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = taker,
        associated_token::authority = taker,
        associated_token::mint = mint_a
    )]
    pub taker_ata_a: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        associated_token::authority = taker,
        associated_token::mint = mint_b,
        associated_token::token_program = token_program
    )]
    pub taker_ata_b: Box<InterfaceAccount<'info, TokenAccount>>,


    #[account(
        init_if_needed,
        payer = taker,
        associated_token::authority = maker,
        associated_token::mint = mint_b,
        associated_token::token_program = token_program
    )]
    pub maker_ata_b: Box<InterfaceAccount<'info, TokenAccount>>,

    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>
}


impl<'info> Take<'info>{
    pub fn transfer_tokens(&mut self)->Result<()>{

        let transfer_b_accounts = TransferChecked{
            authority:self.taker.to_account_info(),
            from: self.taker_ata_b.to_account_info(),
            mint: self.mint_b.to_account_info(),
            to: self.maker_ata_b.to_account_info()
        };

        let transfer_b_context = CpiContext::new(
            self.token_program.to_account_info(),
            transfer_b_accounts
        );

        transfer_checked(transfer_b_context, self.escrow.receive, self.mint_b.decimals)

    }

    pub fn withdraw_and_close_vault(&mut self)->Result<()>{
        let transfer_a_accounts = TransferChecked{
            authority:self.escrow.to_account_info(),
            from: self.vault.to_account_info(),
            mint: self.mint_a.to_account_info(),
            to: self.taker_ata_a.to_account_info()
        };

        let seed_bytes = self.escrow.seed.to_le_bytes();

        let bump_seed = &[self.escrow.bump];

        let signer_seeds = &[&[b"escrow", self.maker.key.as_ref(), &seed_bytes, bump_seed][..]];

        let transfer_a_context = CpiContext::new_with_signer(
            self.token_program.to_account_info(),
            transfer_a_accounts,
            signer_seeds
        );

        transfer_checked(transfer_a_context, self.vault.amount, self.mint_a.decimals)?;

        let close_accounts = CloseAccount{
            account:self.vault.to_account_info(),
            authority:self.escrow.to_account_info(),
            destination:self.maker.to_account_info()
        };

        let close_context = CpiContext::new_with_signer(
            self.token_program.to_account_info(), 
            close_accounts, 
            signer_seeds
        );

        close_account(close_context)
    }
} 