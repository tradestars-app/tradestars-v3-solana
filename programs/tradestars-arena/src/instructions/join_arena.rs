use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token_2022::{self, burn_checked, BurnChecked};
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

use crate::errors::TradestarsArenaError;
use crate::events::ArenaJoined;
use crate::state::{
    ArenaAccount, ArenaPosition, ArenaStatus, UserAccount, MAX_ENTRIES_PER_USER,
};
use crate::utils::{
    fee_amount, ARENA_SEED, POSITION_SEED, TUSDC_DECIMALS, TUSDC_MINT_SEED, USER_SEED,
};

#[derive(Accounts)]
pub struct JoinArena<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [USER_SEED, user.key().as_ref()],
        bump = user_account.bump
    )]
    pub user_account: Box<Account<'info, UserAccount>>,

    #[account(
        mut,
        seeds = [ARENA_SEED, arena.arena_id.as_ref()],
        bump = arena.bump
    )]
    pub arena: Box<Account<'info, ArenaAccount>>,

    #[account(
        init_if_needed,
        payer = user,
        space = ArenaPosition::LEN,
        seeds = [POSITION_SEED, arena.key().as_ref(), user.key().as_ref()],
        bump
    )]
    pub position: Box<Account<'info, ArenaPosition>>,

    #[account(mut, seeds = [TUSDC_MINT_SEED], bump)]
    pub tusdc_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        associated_token::mint = tusdc_mint,
        associated_token::authority = user,
        associated_token::token_program = token_program
    )]
    pub user_tusdc: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<JoinArena>) -> Result<()> {
    require!(
        ctx.accounts.token_program.key() == token_2022::ID,
        TradestarsArenaError::InvalidTokenProgram
    );

    let now = Clock::get()?.unix_timestamp;
    let arena = &mut ctx.accounts.arena;
    require!(arena.status == ArenaStatus::Created, TradestarsArenaError::ArenaNotCreated);
    require!(
        now < arena.entry_close_time,
        TradestarsArenaError::InvalidTime
    );

    let user_account = &mut ctx.accounts.user_account;
    require!(
        user_account.owner == ctx.accounts.user.key(),
        TradestarsArenaError::InvalidUserAccount
    );
    require!(
        user_account.available_to_withdraw()? >= arena.entry_fee,
        TradestarsArenaError::InsufficientAvailableBalance
    );

    let position = &mut ctx.accounts.position;
    let is_first_entry = position.user == Pubkey::default();
    if is_first_entry {
        position.user = ctx.accounts.user.key();
        position.arena = arena.key();
        position.entry_count = 0;
        position.locked_amount = 0;
        position.resolved = false;
        position.last_disputed_settlement_version = 0;
        position.bump = ctx.bumps.position;

        arena.participant_count = arena
            .participant_count
            .checked_add(1)
            .ok_or(TradestarsArenaError::MathOverflow)?;
    } else {
        require!(
            position.user == ctx.accounts.user.key() && position.arena == arena.key(),
            TradestarsArenaError::InvalidArenaPosition
        );
        require!(!position.resolved, TradestarsArenaError::PositionAlreadyResolved);
    }

    require!(
        position.entry_count < MAX_ENTRIES_PER_USER,
        TradestarsArenaError::MaxEntriesReached
    );

    let fee_amount = fee_amount(arena.entry_fee, arena.fee_bps)?;
    let net_entry_amount = arena
        .entry_fee
        .checked_sub(fee_amount)
        .ok_or(TradestarsArenaError::MathOverflow)?;

    burn_checked(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            BurnChecked {
                mint: ctx.accounts.tusdc_mint.to_account_info(),
                from: ctx.accounts.user_tusdc.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ),
        arena.entry_fee,
        TUSDC_DECIMALS,
    )?;

    user_account.in_play_debt = user_account
        .in_play_debt
        .checked_add(arena.entry_fee)
        .ok_or(TradestarsArenaError::MathOverflow)?;
    position.entry_count = position
        .entry_count
        .checked_add(1)
        .ok_or(TradestarsArenaError::MathOverflow)?;
    position.locked_amount = position
        .locked_amount
        .checked_add(arena.entry_fee)
        .ok_or(TradestarsArenaError::MathOverflow)?;

    arena.total_entry_fees_locked = arena
        .total_entry_fees_locked
        .checked_add(arena.entry_fee)
        .ok_or(TradestarsArenaError::MathOverflow)?;
    arena.fee_accrued = arena
        .fee_accrued
        .checked_add(fee_amount)
        .ok_or(TradestarsArenaError::MathOverflow)?;
    arena.total_pool = arena
        .total_pool
        .checked_add(net_entry_amount)
        .ok_or(TradestarsArenaError::MathOverflow)?;

    emit!(ArenaJoined {
        arena: arena.key(),
        user: ctx.accounts.user.key(),
        entry_fee: arena.entry_fee,
        fee_amount,
        net_entry_amount,
        entry_count: position.entry_count,
        locked_amount: position.locked_amount,
        user_in_play_debt: user_account.in_play_debt,
        total_pool: arena.total_pool,
        timestamp: now,
    });

    Ok(())
}
