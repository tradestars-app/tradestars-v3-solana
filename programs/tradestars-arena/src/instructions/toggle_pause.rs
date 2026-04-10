use anchor_lang::prelude::*;

use crate::state::PlatformConfig;

#[derive(Accounts)]
pub struct TogglePause<'info> {
    #[account(
        mut,
        seeds = [b"config"],
        bump = platform_config.bump,
        has_one = authority
    )]
    pub platform_config: Account<'info, PlatformConfig>,

    pub authority: Signer<'info>,
}

pub fn handler(ctx: Context<TogglePause>) -> Result<()> {
    let platform_config = &mut ctx.accounts.platform_config;
    platform_config.paused = !platform_config.paused;
    Ok(())
}
