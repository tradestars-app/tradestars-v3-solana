use anchor_lang::prelude::*;

use crate::errors::TradestarsArenaError;
use crate::state::{Arena, ArenaStatus, PlatformConfig};
use crate::ArenaCancelledEvent;

#[derive(Accounts)]
pub struct CancelArena<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        seeds = [b"config"],
        bump = platform_config.bump,
        has_one = authority
    )]
    pub platform_config: Account<'info, PlatformConfig>,

    #[account(
        mut,
        constraint = arena.authority == authority.key() @ TradestarsArenaError::Unauthorized
    )]
    pub arena: Account<'info, Arena>,
}

pub fn handler(ctx: Context<CancelArena>, reason: String) -> Result<()> {
    let platform_config = &ctx.accounts.platform_config;
    require!(
        !platform_config.paused,
        TradestarsArenaError::PlatformPaused
    );

    let arena = &mut ctx.accounts.arena;
    match arena.status {
        ArenaStatus::Open => arena.status = ArenaStatus::Cancelled,
        ArenaStatus::Cancelled => return Ok(()),
        _ => return err!(TradestarsArenaError::InvalidStatusTransition),
    }

    emit!(ArenaCancelledEvent {
        arena: arena.key(),
        reason,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}
