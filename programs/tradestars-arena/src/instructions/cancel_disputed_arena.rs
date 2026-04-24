use anchor_lang::prelude::*;

use crate::errors::TradestarsArenaError;
use crate::events::ArenaCancelled;
use crate::state::{ArenaAccount, ArenaStatus};
use crate::utils::ARENA_SEED;

#[derive(Accounts)]
pub struct CancelDisputedArena<'info> {
    #[account(
        mut,
        seeds = [ARENA_SEED, arena.arena_id.as_ref()],
        bump = arena.bump
    )]
    pub arena: Account<'info, ArenaAccount>,

    pub caller: Signer<'info>,
}

pub fn handler(ctx: Context<CancelDisputedArena>) -> Result<()> {
    let arena = &mut ctx.accounts.arena;
    require!(
        arena.status == ArenaStatus::Disputed,
        TradestarsArenaError::ArenaNotDisputed
    );

    arena.status = ArenaStatus::Cancelled;

    emit!(ArenaCancelled {
        arena: arena.key(),
        creator: arena.creator,
        cancelled_by: ctx.accounts.caller.key(),
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}
