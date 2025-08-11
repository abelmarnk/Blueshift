use anchor_lang::prelude::*;
use anchor_spl::{associated_token::AssociatedToken, token_interface::{Mint, TokenAccount, TokenInterface, transfer_checked, TransferChecked}};
use crate::state::Escrow;

#[derive(Accounts)]
#[instruction(seed:u64)]
pub struct Make<'info> {
    #[account(
        mut
    )]
    pub maker:Signer<'info>,

    #[account(
        init,
        payer = maker,
        space = Escrow::DISCRIMINATOR.len() + Escrow::INIT_SPACE,
        seeds = [b"escrow", maker.key().as_ref(), seed.to_le_bytes().as_ref()],
        bump
    )]
    pub escrow: Account<'info, Escrow>,

    #[account(
        owner = token_program.key()
    )]
    pub mint_a:InterfaceAccount<'info, Mint>,

    #[account(
        owner = token_program.key()
    )]
    pub mint_b:InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        associated_token::authority = maker,
        associated_token::mint = mint_a,
        associated_token::token_program = token_program
    )]
    pub maker_ata_a: InterfaceAccount<'info, TokenAccount>,

    #[account(
        init,
        payer = maker,
        associated_token::authority = escrow,
        associated_token::mint = mint_a,
        associated_token::token_program = token_program
    )]
    pub vault: InterfaceAccount<'info, TokenAccount>,

    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>
}

impl<'info> Make<'info>  {
    
pub fn populate_escrow(&mut self, seed:u64, amount_expected:u64, bump:u8){
    self.escrow.set_inner(
        Escrow { 
            seed, 
            maker: *self.maker.key, 
            mint_a: self.mint_a.key(), 
            mint_b: self.mint_b.key(), 
            receive: amount_expected, 
            bump
        }
    );
}

pub fn transfer_tokens(&mut self, amount_deposited:u64) ->Result<()>{

    let accounts = TransferChecked{
        authority:self.maker.to_account_info(),
        from: self.maker_ata_a.to_account_info(),
        mint: self.mint_a.to_account_info(),
        to: self.vault.to_account_info(),
    };

    let context = CpiContext::new(
        self.token_program.to_account_info(),
        accounts
    );

    transfer_checked(
        context,
        amount_deposited,
        self.mint_a.decimals
    )
}
}