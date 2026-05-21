use anchor_lang::prelude::*;

use crate::errors::TradestarsArenaError;
use crate::events::WalletDepositConfigUpdated;
use crate::state::{PlatformConfig, WalletDepositConfig};
use crate::utils::{require_authority, CONFIG_SEED, WALLET_DEPOSIT_CONFIG_SEED};

#[derive(Accounts)]
pub struct SetWalletDepositConfig<'info> {
    #[account(seeds = [CONFIG_SEED], bump = platform_config.bump)]
    pub platform_config: Account<'info, PlatformConfig>,

    #[account(
        init_if_needed,
        payer = authority,
        space = WalletDepositConfig::LEN,
        seeds = [WALLET_DEPOSIT_CONFIG_SEED],
        bump
    )]
    pub wallet_deposit_config: Account<'info, WalletDepositConfig>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<SetWalletDepositConfig>, usdc_mint: Pubkey) -> Result<()> {
    require_authority(&ctx.accounts.platform_config, &ctx.accounts.authority.key())?;
    require!(
        usdc_mint != Pubkey::default(),
        TradestarsArenaError::InvalidCollateralMint
    );

    let wallet_deposit_config = &mut ctx.accounts.wallet_deposit_config;
    wallet_deposit_config.usdc_mint = usdc_mint;
    wallet_deposit_config.bump = ctx.bumps.wallet_deposit_config;

    emit!(WalletDepositConfigUpdated {
        usdc_mint,
        authority: ctx.accounts.authority.key(),
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}
