use anchor_lang::prelude::*;
use anchor_lang::solana_program::account_info::next_account_info;
use anchor_spl::token::{transfer, Token, TokenAccount, Transfer};

use crate::errors::TradestarsArenaError;
use crate::state::{Arena, ArenaEntry, ArenaStatus, PlatformConfig};
use crate::RefundClaimedEvent;

#[derive(Accounts)]
pub struct RefundBatch<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        seeds = [b"config"],
        bump = platform_config.bump,
        has_one = authority
    )]
    pub platform_config: Account<'info, PlatformConfig>,

    #[account(mut)]
    pub arena: Account<'info, Arena>,

    #[account(
        mut,
        seeds = [b"vault", arena.key().as_ref()],
        bump,
        constraint = arena_vault.owner == arena.key()
    )]
    pub arena_vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

pub fn handler<'info>(ctx: Context<'_, '_, 'info, 'info, RefundBatch<'info>>) -> Result<()> {
    let platform_config = &ctx.accounts.platform_config;
    require!(
        !platform_config.paused,
        TradestarsArenaError::PlatformPaused
    );

    require!(
        ctx.accounts.arena.status == ArenaStatus::Cancelled,
        TradestarsArenaError::ArenaNotCancelled
    );

    require!(
        ctx.remaining_accounts.len() % 2 == 0,
        TradestarsArenaError::InvalidRemainingAccounts
    );

    let arena_id = ctx.accounts.arena.arena_id.clone();
    let arena_bump = ctx.accounts.arena.bump;
    let arena_key = ctx.accounts.arena.key();
    let signer_seeds = &[b"arena".as_ref(), arena_id.as_bytes(), &[arena_bump]];
    let signer = &[&signer_seeds[..]];

    let mut expected_vault_balance = ctx.accounts.arena_vault.amount;
    let mut total_refunds_paid = ctx.accounts.arena.total_refunds_paid;

    let mut remaining_accounts_iter = ctx.remaining_accounts.iter();
    for _ in 0..(ctx.remaining_accounts.len() / 2) {
        let entry_info = next_account_info(&mut remaining_accounts_iter)?;
        let user_usdc_info = next_account_info(&mut remaining_accounts_iter)?;

        require!(
            entry_info.is_writable,
            TradestarsArenaError::InvalidRemainingAccounts
        );

        let mut entry = Account::<ArenaEntry>::try_from(entry_info)?;
        require!(
            entry.arena == arena_key,
            TradestarsArenaError::InvalidRemainingAccounts
        );

        let user_usdc = Account::<TokenAccount>::try_from(user_usdc_info)?;
        require!(
            user_usdc.owner == entry.user && user_usdc.mint == ctx.accounts.arena_vault.mint,
            TradestarsArenaError::InvalidWinnerTokenAccount
        );

        if entry.settled {
            entry.exit(ctx.program_id)?;
            continue;
        }

        let refund_amount = entry.amount_paid;
        require!(refund_amount > 0, TradestarsArenaError::AmountTooSmall);

        total_refunds_paid = total_refunds_paid
            .checked_add(refund_amount)
            .ok_or(TradestarsArenaError::MathOverflow)?;
        require!(
            total_refunds_paid <= ctx.accounts.arena.total_pool,
            TradestarsArenaError::MathOverflow
        );

        require!(
            expected_vault_balance >= refund_amount,
            TradestarsArenaError::InsufficientFunds
        );

        transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.arena_vault.to_account_info(),
                    to: user_usdc_info.to_account_info(),
                    authority: ctx.accounts.arena.to_account_info(),
                },
                signer,
            ),
            refund_amount,
        )?;

        expected_vault_balance = expected_vault_balance
            .checked_sub(refund_amount)
            .ok_or(TradestarsArenaError::MathOverflow)?;

        entry.settled = true;
        entry.payout_amount = 0;

        let entry_key = entry.key();
        let entry_user = entry.user;
        entry.exit(ctx.program_id)?;

        emit!(RefundClaimedEvent {
            arena: arena_key,
            entry: entry_key,
            user: entry_user,
            amount: refund_amount,
            total_refunds_paid,
            timestamp: Clock::get()?.unix_timestamp,
        });
    }

    ctx.accounts.arena.total_refunds_paid = total_refunds_paid;

    Ok(())
}
