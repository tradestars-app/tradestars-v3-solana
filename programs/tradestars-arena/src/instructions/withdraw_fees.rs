use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};

use crate::errors::TradestarsArenaError;
use crate::state::PlatformConfig;

#[derive(Accounts)]
pub struct WithdrawFees<'info> {
    #[account(
        seeds = [b"config"],
        bump = platform_config.bump,
        has_one = authority
    )]
    pub platform_config: Account<'info, PlatformConfig>,

    #[account(
        mut,
        seeds = [b"platform_vault"],
        bump
    )]
    pub platform_vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = destination.owner == platform_config.fee_recipient,
        constraint = destination.mint == platform_vault.mint @ TradestarsArenaError::InvalidTokenAccount
    )]
    pub destination: Account<'info, TokenAccount>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub token_program: Program<'info, Token>,
}

pub fn handler(ctx: Context<WithdrawFees>, amount: Option<u64>) -> Result<()> {
    let platform_vault = &ctx.accounts.platform_vault;
    let withdraw_amount = amount.unwrap_or(platform_vault.amount);

    require!(withdraw_amount > 0, TradestarsArenaError::AmountTooSmall);
    require!(
        withdraw_amount <= platform_vault.amount,
        TradestarsArenaError::InsufficientFunds
    );

    let platform_config = &ctx.accounts.platform_config;
    let seeds = &[b"config".as_ref(), &[platform_config.bump]];
    let signer = &[&seeds[..]];

    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.platform_vault.to_account_info(),
                to: ctx.accounts.destination.to_account_info(),
                authority: platform_config.to_account_info(),
            },
            signer,
        ),
        withdraw_amount,
    )?;

    Ok(())
}
