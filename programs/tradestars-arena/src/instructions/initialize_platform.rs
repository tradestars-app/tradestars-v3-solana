use anchor_lang::prelude::*;
use anchor_spl::token_2022::{self, initialize_mint2, InitializeMint2};
use anchor_spl::token_2022_extensions::{
    non_transferable_mint_initialize, NonTransferableMintInitialize,
};
use anchor_spl::token_interface::TokenInterface;

use crate::errors::TradestarsArenaError;
use crate::state::PlatformConfig;
use crate::utils::{
    create_pda_account, tusdc_mint_space, CONFIG_SEED, TUSDC_DECIMALS, TUSDC_MINT_SEED,
};

#[derive(Accounts)]
pub struct InitializePlatform<'info> {
    #[account(
        init,
        payer = authority,
        space = PlatformConfig::LEN,
        seeds = [CONFIG_SEED],
        bump
    )]
    pub platform_config: Account<'info, PlatformConfig>,

    #[account(mut, seeds = [TUSDC_MINT_SEED], bump)]
    /// CHECK: initialized as a Token-2022 mint in the handler.
    pub tusdc_mint: UncheckedAccount<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<InitializePlatform>,
    arena_operator: Pubkey,
    treasury_wallet: Pubkey,
    minting_authority: Pubkey,
    dispute_window_seconds: u64,
    settlement_grace_period_seconds: u64,
) -> Result<()> {
    require!(
        ctx.accounts.token_program.key() == token_2022::ID,
        TradestarsArenaError::InvalidTokenProgram
    );
    require!(
        arena_operator != Pubkey::default(),
        TradestarsArenaError::InvalidRoleKey
    );
    require!(dispute_window_seconds > 0, TradestarsArenaError::InvalidCooldown);
    require!(
        settlement_grace_period_seconds > 0,
        TradestarsArenaError::InvalidCooldown
    );
    require!(
        treasury_wallet != Pubkey::default(),
        TradestarsArenaError::InvalidTreasuryWallet
    );
    require!(
        minting_authority != Pubkey::default(),
        TradestarsArenaError::InvalidRoleKey
    );

    let mint_space = tusdc_mint_space()?;
    let mint_bump = ctx.bumps.tusdc_mint;
    let mint_signer_seeds: &[&[u8]] = &[TUSDC_MINT_SEED, &[mint_bump]];

    create_pda_account(
        &ctx.accounts.authority.to_account_info(),
        &ctx.accounts.tusdc_mint.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        &ctx.accounts.token_program.key(),
        mint_space,
        mint_signer_seeds,
    )?;

    non_transferable_mint_initialize(CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        NonTransferableMintInitialize {
            token_program_id: ctx.accounts.token_program.to_account_info(),
            mint: ctx.accounts.tusdc_mint.to_account_info(),
        },
    ))?;

    initialize_mint2(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            InitializeMint2 {
                mint: ctx.accounts.tusdc_mint.to_account_info(),
            },
        ),
        TUSDC_DECIMALS,
        &ctx.accounts.platform_config.key(),
        None,
    )?;

    ctx.accounts.platform_config.set_inner(PlatformConfig {
        authority: ctx.accounts.authority.key(),
        arena_operator,
        treasury_wallet,
        minting_authority,
        dispute_window_seconds,
        settlement_grace_period_seconds,
        deposits_paused: false,
        bump: ctx.bumps.platform_config,
    });

    Ok(())
}
