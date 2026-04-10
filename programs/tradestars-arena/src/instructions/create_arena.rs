use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

use crate::errors::TradestarsArenaError;
use crate::state::{Arena, ArenaStatus, PlatformConfig};
use crate::ArenaCreatedEvent;

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct CreateArenaParams {
    pub entry_fee: u64,
    pub guaranteed_prize_pool: u64,
    pub start_time: i64,
    pub end_time: i64,
    pub max_entries_per_user: u8,
}

#[derive(Accounts)]
#[instruction(arena_id: String)]
pub struct CreateArena<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        seeds = [b"config"],
        bump = platform_config.bump,
        has_one = authority
    )]
    pub platform_config: Account<'info, PlatformConfig>,

    #[account(
        init,
        payer = authority,
        space = Arena::LEN,
        seeds = [b"arena", arena_id.as_bytes()],
        bump
    )]
    pub arena: Account<'info, Arena>,

    pub usdc_mint: Account<'info, Mint>,

    #[account(
        init,
        payer = authority,
        seeds = [b"vault", arena.key().as_ref()],
        bump,
        token::mint = usdc_mint,
        token::authority = arena
    )]
    pub arena_vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn handler(
    ctx: Context<CreateArena>,
    arena_id: String,
    params: CreateArenaParams,
) -> Result<()> {
    require!(
        !arena_id.is_empty() && arena_id.len() <= Arena::MAX_ARENA_ID_LEN,
        TradestarsArenaError::InvalidArenaId
    );

    let platform_config = &ctx.accounts.platform_config;
    require!(
        !platform_config.paused,
        TradestarsArenaError::PlatformPaused
    );
    require!(params.entry_fee > 0, TradestarsArenaError::AmountTooSmall);
    require!(
        params.guaranteed_prize_pool > 0,
        TradestarsArenaError::AmountTooSmall
    );
    require!(
        params.max_entries_per_user > 0,
        TradestarsArenaError::InvalidEntryNumber
    );
    require!(
        params.start_time < params.end_time,
        TradestarsArenaError::InvalidTime
    );
    let now = Clock::get()?.unix_timestamp;
    require!(
        params.end_time > now,
        TradestarsArenaError::InvalidTime
    );

    let arena = &mut ctx.accounts.arena;
    arena.arena_id = arena_id.clone();
    arena.entry_fee = params.entry_fee;
    arena.guaranteed_prize_pool = params.guaranteed_prize_pool;
    arena.start_time = params.start_time;
    arena.end_time = params.end_time;
    arena.total_entries = 0;
    arena.total_pool = 0;
    arena.fee_amount = 0;
    arena.fee_collected = false;
    arena.overlay_funded = 0;
    arena.total_payouts_set = 0;
    arena.total_refunds_paid = 0;
    arena.status = ArenaStatus::Open;
    arena.max_entries_per_user = params.max_entries_per_user;
    arena.authority = ctx.accounts.authority.key();
    arena.creator = ctx.accounts.authority.key();
    arena.bump = ctx.bumps.arena;

    emit!(ArenaCreatedEvent {
        arena: arena.key(),
        arena_id,
        entry_fee: arena.entry_fee,
        guaranteed_prize_pool: arena.guaranteed_prize_pool,
        start_time: arena.start_time,
        end_time: arena.end_time,
        creator: arena.creator,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}
