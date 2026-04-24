use anchor_lang::prelude::*;

use crate::errors::TradestarsArenaError;
use crate::events::SettlementRootPosted;
use crate::state::{ArenaAccount, ArenaStatus, PlatformConfig};
use crate::utils::{require_arena_operator, ARENA_SEED, CONFIG_SEED};

#[derive(Accounts)]
pub struct PostSettlementRoot<'info> {
    #[account(seeds = [CONFIG_SEED], bump = platform_config.bump)]
    pub platform_config: Account<'info, PlatformConfig>,

    #[account(
        mut,
        seeds = [ARENA_SEED, arena.arena_id.as_ref()],
        bump = arena.bump
    )]
    pub arena: Account<'info, ArenaAccount>,

    pub authority: Signer<'info>,
}

pub fn handler(
    ctx: Context<PostSettlementRoot>,
    merkle_root: [u8; 32],
) -> Result<()> {
    require_arena_operator(&ctx.accounts.platform_config, &ctx.accounts.authority.key())?;

    let arena = &mut ctx.accounts.arena;
    require!(arena.status == ArenaStatus::Created, TradestarsArenaError::ArenaNotCreated);

    let now = Clock::get()?.unix_timestamp;
    require!(now >= arena.end_time, TradestarsArenaError::InvalidTime);

    let cooldown = i64::try_from(ctx.accounts.platform_config.dispute_window_seconds)
        .map_err(|_| TradestarsArenaError::InvalidCooldown)?;
    let next_version = arena
        .settlement_version
        .checked_add(1)
        .ok_or(TradestarsArenaError::MathOverflow)?;

    arena.merkle_root = merkle_root;
    arena.settlement_timestamp = now;
    arena.claimable_at = now
        .checked_add(cooldown)
        .ok_or(TradestarsArenaError::MathOverflow)?;
    arena.dispute_count = 0;
    arena.settlement_version = next_version;
    arena.status = ArenaStatus::SettledPendingClaim;

    emit!(SettlementRootPosted {
        arena: arena.key(),
        settlement_version: next_version,
        merkle_root,
        settlement_timestamp: arena.settlement_timestamp,
        claimable_at: arena.claimable_at,
    });

    Ok(())
}
