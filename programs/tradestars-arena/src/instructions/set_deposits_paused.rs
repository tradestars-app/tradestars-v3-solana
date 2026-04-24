use anchor_lang::prelude::*;

use crate::events::DepositsPauseUpdated;
use crate::state::PlatformConfig;
use crate::utils::{require_authority, CONFIG_SEED};

#[derive(Accounts)]
pub struct SetDepositsPaused<'info> {
    #[account(mut, seeds = [CONFIG_SEED], bump = platform_config.bump)]
    pub platform_config: Account<'info, PlatformConfig>,

    pub authority: Signer<'info>,
}

pub fn handler(ctx: Context<SetDepositsPaused>, paused: bool) -> Result<()> {
    require_authority(&ctx.accounts.platform_config, &ctx.accounts.authority.key())?;

    ctx.accounts.platform_config.deposits_paused = paused;

    emit!(DepositsPauseUpdated {
        paused,
        authority: ctx.accounts.authority.key(),
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}
