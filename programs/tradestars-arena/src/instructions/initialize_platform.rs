use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

use crate::errors::TradestarsArenaError;
use crate::state::PlatformConfig;

#[derive(Accounts)]
pub struct InitializePlatform<'info> {
    #[account(
        init,
        payer = authority,
        space = PlatformConfig::LEN,
        seeds = [b"config"],
        bump
    )]
    pub platform_config: Account<'info, PlatformConfig>,

    #[account(
        init,
        payer = authority,
        seeds = [b"platform_vault"],
        bump,
        token::mint = usdc_mint,
        token::authority = platform_config,
    )]
    pub platform_vault: Account<'info, TokenAccount>,

    pub usdc_mint: Account<'info, Mint>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<InitializePlatform>,
    fee_recipient: Pubkey,
    platform_fee_bps: u16,
) -> Result<()> {
    require!(
        platform_fee_bps <= 10_000,
        TradestarsArenaError::InvalidFeeBps
    );

    let platform_config = &mut ctx.accounts.platform_config;
    platform_config.authority = ctx.accounts.authority.key();
    platform_config.fee_recipient = fee_recipient;
    platform_config.platform_fee_bps = platform_fee_bps;
    platform_config.paused = false;
    platform_config.bump = ctx.bumps.platform_config;

    Ok(())
}
