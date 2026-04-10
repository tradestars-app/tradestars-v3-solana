use anchor_lang::prelude::*;
use anchor_spl::token::{transfer, Mint, Token, TokenAccount, Transfer};

use crate::errors::TradestarsArenaError;
use crate::state::{Arena, ArenaEntry, ArenaStatus, PlatformConfig};
use crate::EntryEvent;

#[derive(Accounts)]
#[instruction(entry_number: u8)]
pub struct EnterArena<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        seeds = [b"config"],
        bump = platform_config.bump
    )]
    pub platform_config: Account<'info, PlatformConfig>,

    #[account(mut)]
    pub arena: Account<'info, Arena>,

    #[account(
        init,
        payer = user,
        space = ArenaEntry::LEN,
        seeds = [b"entry", arena.key().as_ref(), user.key().as_ref(), &[entry_number]],
        bump
    )]
    pub entry: Account<'info, ArenaEntry>,

    pub usdc_mint: Account<'info, Mint>,

    #[account(
        mut,
        constraint = user_usdc.owner == user.key(),
        constraint = user_usdc.mint == usdc_mint.key()
    )]
    pub user_usdc: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [b"vault", arena.key().as_ref()],
        bump,
        constraint = arena_vault.owner == arena.key(),
        constraint = arena_vault.mint == usdc_mint.key()
    )]
    pub arena_vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<EnterArena>, entry_number: u8) -> Result<()> {
    let platform_config = &ctx.accounts.platform_config;
    require!(
        !platform_config.paused,
        TradestarsArenaError::PlatformPaused
    );

    let arena = &mut ctx.accounts.arena;
    let now = Clock::get()?.unix_timestamp;
    require!(
        arena.status == ArenaStatus::Open,
        TradestarsArenaError::ArenaNotOpen
    );
    require!(
        now < arena.end_time,
        TradestarsArenaError::EntryWindowClosed
    );
    require!(
        entry_number < arena.max_entries_per_user,
        TradestarsArenaError::EntryLimitReached
    );

    let amount = arena.entry_fee;
    require!(amount > 0, TradestarsArenaError::AmountTooSmall);

    transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.user_usdc.to_account_info(),
                to: ctx.accounts.arena_vault.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ),
        amount,
    )?;

    let entry = &mut ctx.accounts.entry;
    entry.user = ctx.accounts.user.key();
    entry.arena = arena.key();
    entry.entry_number = entry_number;
    entry.amount_paid = amount;
    entry.payout_amount = 0;
    entry.settled = false;
    entry.bump = ctx.bumps.entry;

    arena.total_entries = arena
        .total_entries
        .checked_add(1)
        .ok_or(TradestarsArenaError::MathOverflow)?;
    arena.total_pool = arena
        .total_pool
        .checked_add(amount)
        .ok_or(TradestarsArenaError::MathOverflow)?;

    emit!(EntryEvent {
        arena: arena.key(),
        entry: entry.key(),
        user: entry.user,
        entry_number,
        amount_paid: amount,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}
