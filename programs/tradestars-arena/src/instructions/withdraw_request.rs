use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token_2022::{self, burn_checked, BurnChecked};
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

use crate::errors::TradestarsArenaError;
use crate::events::WithdrawRequested;
use crate::state::UserAccount;
use crate::utils::{TUSDC_DECIMALS, TUSDC_MINT_SEED, USER_SEED};

#[derive(Accounts)]
pub struct WithdrawRequest<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [USER_SEED, user.key().as_ref()],
        bump = user_account.bump
    )]
    pub user_account: Account<'info, UserAccount>,

    #[account(mut, seeds = [TUSDC_MINT_SEED], bump)]
    pub tusdc_mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = tusdc_mint,
        associated_token::authority = user,
        associated_token::token_program = token_program
    )]
    pub user_tusdc: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

pub fn handler(ctx: Context<WithdrawRequest>, amount: u64, nonce: u64) -> Result<()> {
    require!(amount > 0, TradestarsArenaError::AmountTooSmall);
    require!(
        ctx.accounts.token_program.key() == token_2022::ID,
        TradestarsArenaError::InvalidTokenProgram
    );

    let user_account = &mut ctx.accounts.user_account;
    require!(
        user_account.owner == ctx.accounts.user.key(),
        TradestarsArenaError::InvalidUserAccount
    );
    require!(
        user_account
            .last_nonce
            .checked_add(1)
            .ok_or(TradestarsArenaError::MathOverflow)?
            == nonce,
        TradestarsArenaError::InvalidNonce
    );

    let available_to_withdraw = user_account.available_to_withdraw()?;
    require!(
        amount <= available_to_withdraw,
        TradestarsArenaError::InsufficientAvailableBalance
    );

    burn_checked(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            BurnChecked {
                mint: ctx.accounts.tusdc_mint.to_account_info(),
                from: ctx.accounts.user_tusdc.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ),
        amount,
        TUSDC_DECIMALS,
    )?;

    user_account.total_balance = user_account
        .total_balance
        .checked_sub(amount)
        .ok_or(TradestarsArenaError::MathOverflow)?;
    user_account.last_nonce = nonce;

    let available_after = user_account.available_to_withdraw()?;

    emit!(WithdrawRequested {
        user: ctx.accounts.user.key(),
        amount,
        nonce,
        remaining_total_balance: user_account.total_balance,
        available_to_withdraw: available_after,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}
