use anchor_lang::prelude::*;

use crate::errors::TradestarsArenaError;
use crate::events::ArenaDisputed;
use crate::state::{
    ArenaAccount, ArenaPosition, ArenaStatus, PlatformConfig, DISPUTE_THRESHOLD_BPS,
};
use crate::utils::{dispute_threshold_reached, ARENA_SEED, CONFIG_SEED, POSITION_SEED};

#[derive(Accounts)]
pub struct SubmitDispute<'info> {
    #[account(seeds = [CONFIG_SEED], bump = platform_config.bump)]
    pub platform_config: Account<'info, PlatformConfig>,

    #[account(
        mut,
        seeds = [ARENA_SEED, arena.arena_id.as_ref()],
        bump = arena.bump
    )]
    pub arena: Account<'info, ArenaAccount>,

    #[account(
        mut,
        seeds = [POSITION_SEED, arena.key().as_ref(), user.key().as_ref()],
        bump = position.bump
    )]
    pub position: Account<'info, ArenaPosition>,

    #[account(mut)]
    pub user: Signer<'info>,
}

pub fn handler(ctx: Context<SubmitDispute>) -> Result<()> {
    let arena = &mut ctx.accounts.arena;
    require!(
        arena.status == ArenaStatus::SettledPendingClaim,
        TradestarsArenaError::ArenaNotSettled
    );
    require!(
        ctx.accounts.position.user == ctx.accounts.user.key()
            && ctx.accounts.position.arena == arena.key(),
        TradestarsArenaError::InvalidArenaPosition
    );
    require!(
        ctx.accounts.position.entry_count > 0 && !ctx.accounts.position.resolved,
        TradestarsArenaError::NotArenaParticipant
    );
    require!(
        ctx.accounts.position.last_disputed_settlement_version != arena.settlement_version,
        TradestarsArenaError::AlreadyDisputed
    );

    let now = Clock::get()?.unix_timestamp;
    require!(now < arena.claimable_at, TradestarsArenaError::InvalidStatusTransition);

    ctx.accounts.position.last_disputed_settlement_version = arena.settlement_version;
    arena.dispute_count = arena
        .dispute_count
        .checked_add(1)
        .ok_or(TradestarsArenaError::MathOverflow)?;
    let disputed = dispute_threshold_reached(arena.dispute_count, arena.participant_count)?;
    if disputed {
        arena.status = ArenaStatus::Disputed;
    }

    emit!(ArenaDisputed {
        arena: arena.key(),
        user: ctx.accounts.user.key(),
        settlement_version: arena.settlement_version,
        dispute_count: arena.dispute_count,
        participant_count: arena.participant_count,
        threshold_bps: DISPUTE_THRESHOLD_BPS,
        disputed,
        timestamp: now,
    });

    Ok(())
}
