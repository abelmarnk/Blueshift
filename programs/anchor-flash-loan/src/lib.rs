use anchor_lang::prelude::*;
use anchor_lang::{
    solana_program::{
        sysvar::{
            instructions::{
                ID as SYSVAR_INSTRUCTIONS_ID,
                load_current_index_checked,
                load_instruction_at_checked,
            }
        }
    }
};
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{
        Token,
        Transfer,
        transfer,
        Mint, 
        TokenAccount
    }
};

declare_id!("22222222222222222222222222222222222222222222");

#[program]
pub mod anchor_flash_loan {
    use super::*;

    pub fn borrow(ctx: Context<Loan>, amount:u64) -> Result<()> {
        // Check if the amount is valid
        require_gt!(amount, 0, ProtocolError::InvalidAmount);

        // Check if this is the first instruction in the transaction.
        let current_index = load_current_index_checked(&ctx.accounts.sysvar_instructions)?;
        require_eq!(current_index, 0, ProtocolError::InvalidIx); 

        // Get the count of instructions in the transaction
        let instruction_count = u16::from_le_bytes(
            ctx.accounts.sysvar_instructions.data.borrow()[..2].try_into().unwrap());

        // Get the repay instruction
        let repay_instruction = 
            load_instruction_at_checked(instruction_count as usize - 1, 
                &ctx.accounts.sysvar_instructions).map_err(|_| ProtocolError::MissingRepayIx)?;

        // Affirm the keys
        require_keys_eq!(crate::ID, repay_instruction.program_id, ProtocolError::InvalidProgram);
        
        // Affirm the instruction
        require!(repay_instruction.data.as_slice()[0..8].eq(instruction::Repay::DISCRIMINATOR), ProtocolError::InvalidIx);

        // Affirm the accounts
        require_keys_eq!(repay_instruction.accounts.get(3).
            ok_or_else(|| ProtocolError::InvalidBorrowerAta)?.pubkey, 
            ctx.accounts.borrower_ata.key(), ProtocolError::InvalidBorrowerAta);

        require_keys_eq!(repay_instruction.accounts.get(4).
            ok_or_else(|| ProtocolError::InvalidProtocolAta)?.pubkey, 
            ctx.accounts.protocol_ata.key(), ProtocolError::InvalidProtocolAta);

        // Make the transfer

        let transfer_accounts = Transfer{
            from:ctx.accounts.protocol_ata.to_account_info(),
            to:ctx.accounts.borrower_ata.to_account_info(),
            authority:ctx.accounts.protocol.to_account_info()
        };

        let seeds = [b"protocol".as_ref(), &[ctx.bumps.protocol]];

        let signer = [&seeds[..]];

        let transfer_context = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            transfer_accounts,
            &signer
        );

        transfer(transfer_context, amount)
    }

    pub fn repay(ctx: Context<Loan>) -> Result<()> {
        // Get the borrow amount from the first instruction in the transaction
        let borrow_instruction = 
            load_instruction_at_checked(0, &ctx.accounts.sysvar_instructions).
            map_err(|_| ProtocolError::MissingBorrowIx)?;
        
        // Get the amount
        let mut amount = u64::from_le_bytes(borrow_instruction.data.as_slice()[8..16].try_into().unwrap());

        // Make the tranfer
        let fee = u64::try_from((amount as u128).checked_mul(500).
            ok_or_else(|| ProtocolError::Overflow)?.checked_div(10_000).
            ok_or_else(|| ProtocolError::Overflow)?).map_err(|_| ProtocolError::Overflow)?;

        amount = amount.checked_add(fee).ok_or_else(|| ProtocolError::Overflow)?;

        let transfer_accounts = Transfer{
            from:ctx.accounts.borrower_ata.to_account_info(),
            to:ctx.accounts.protocol_ata.to_account_info(),
            authority: ctx.accounts.borrower.to_account_info()
        };

        let transfer_context = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            transfer_accounts,
        );

        transfer(transfer_context, amount)
    }
}

#[derive(Accounts)]
pub struct Loan<'info>{

    #[account(
        mut
    )]
    borrower:Signer<'info>,

    #[account(
        seeds = [b"protocol"],
        bump
    )]
    /// CHECK: "unsafe" tastes better
    protocol:UncheckedAccount<'info>,

    mint:Account<'info, Mint>,

    #[account(
        init_if_needed,
        payer = borrower,
        associated_token::mint = mint,
        associated_token::authority = borrower
    )]
    borrower_ata:Account<'info, TokenAccount>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = protocol
    )]
    protocol_ata:Account<'info, TokenAccount>,

    #[account(
        address = SYSVAR_INSTRUCTIONS_ID
    )]
    /// CHECK: Address is checked above
    sysvar_instructions:UncheckedAccount<'info>,
    
    token_program:Program<'info, Token>,

    associated_token_program:Program<'info, AssociatedToken>,

    system_program:Program<'info, System>
}


#[error_code]
pub enum ProtocolError {
    #[msg("Invalid instruction")]
    InvalidIx,
    #[msg("Invalid instruction index")]
    InvalidInstructionIndex,
    #[msg("Invalid amount")]
    InvalidAmount,
    #[msg("Not enough funds")]
    NotEnoughFunds,
    #[msg("Program Mismatch")]
    ProgramMismatch,
    #[msg("Invalid program")]
    InvalidProgram,
    #[msg("Invalid borrower ATA")]
    InvalidBorrowerAta,
    #[msg("Invalid protocol ATA")]
    InvalidProtocolAta,
    #[msg("Missing repay instruction")]
    MissingRepayIx,
    #[msg("Missing borrow instruction")]
    MissingBorrowIx,
    #[msg("Overflow")]
    Overflow,
}