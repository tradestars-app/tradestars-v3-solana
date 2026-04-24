use anchor_lang::prelude::*;

use crate::errors::TradestarsArenaError;
use crate::events::ArenaCancelled;
use crate::state::{ArenaAccount, ArenaStatus, PlatformConfig};
use crate::utils::{ARENA_SEED, CONFIG_SEED};

#[derive(Accounts)]
pub struct CancelStaleArena<'info> {
    #[account(seeds = [CONFIG_SEED], bump = platform_config.bump)]
    pub platform_config: Account<'info, PlatformConfig>,

    #[account(
        mut,
        seeds = [ARENA_SEED, arena.arena_id.as_ref()],
        bump = arena.bump
    )]
    pub arena: Account<'info, ArenaAccount>,

    pub caller: Signer<'info>,
}

pub fn handler(ctx: Context<CancelStaleArena>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let arena = &mut ctx.accounts.arena;
    require!(arena.status == ArenaStatus::Created, TradestarsArenaError::ArenaNotCreated);

    let stale_cutoff = arena
        .end_time
        .checked_add(
            i64::try_from(ctx.accounts.platform_config.settlement_grace_period_seconds)
                .map_err(|_| TradestarsArenaError::InvalidCooldown)?,
        )
        .ok_or(TradestarsArenaError::MathOverflow)?;
    require!(now >= stale_cutoff, TradestarsArenaError::ArenaNotStale);

    arena.status = ArenaStatus::Cancelled;

    emit!(ArenaCancelled {
        arena: arena.key(),
        creator: arena.creator,
        cancelled_by: ctx.accounts.caller.key(),
        timestamp: now,
    });

    Ok(())
}
