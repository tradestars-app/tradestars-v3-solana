use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token_2022::{self, mint_to_checked, MintToChecked};
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

use crate::errors::TradestarsArenaError;
use crate::events::CollateralDeposited;
use crate::state::{Marker, PlatformConfig, UserAccount};
use crate::utils::{
    verify_deposit_attestation, CONFIG_SEED, DEPOSIT_SEED, TUSDC_DECIMALS, TUSDC_MINT_SEED,
    USER_SEED,
};

#[derive(Accounts)]
#[instruction(amount: u64, base_tx_hash: [u8; 32], log_index: u32, signature: [u8; 64], recovery_id: u8)]
pub struct DepositCollateral<'info> {
    #[account(seeds = [CONFIG_SEED], bump = platform_config.bump)]
    pub platform_config: Account<'info, PlatformConfig>,

    #[account(mut, seeds = [TUSDC_MINT_SEED], bump)]
    pub tusdc_mint: InterfaceAccount<'info, Mint>,

    /// CHECK: user wallet receiving tUSDC.
    pub user: UncheckedAccount<'info>,

    #[account(
        init_if_needed,
        payer = submitter,
        space = UserAccount::LEN,
        seeds = [USER_SEED, user.key().as_ref()],
        bump
    )]
    pub user_account: Account<'info, UserAccount>,

    #[account(
        init,
        payer = submitter,
        space = Marker::LEN,
        seeds = [DEPOSIT_SEED, base_tx_hash.as_ref(), &log_index.to_le_bytes()],
        bump
    )]
    pub deposit_marker: Account<'info, Marker>,

    #[account(
        init_if_needed,
        payer = submitter,
        associated_token::mint = tusdc_mint,
        associated_token::authority = user,
        associated_token::token_program = token_program
    )]
    pub user_tusdc: InterfaceAccount<'info, TokenAccount>,

    #[account(mut)]
    pub submitter: Signer<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<DepositCollateral>,
    amount: u64,
    base_tx_hash: [u8; 32],
    log_index: u32,
    signature: [u8; 64],
    recovery_id: u8,
) -> Result<()> {
    require!(amount > 0, TradestarsArenaError::AmountTooSmall);
    require!(
        ctx.accounts.token_program.key() == token_2022::ID,
        TradestarsArenaError::InvalidTokenProgram
    );

    require!(
        !ctx.accounts.platform_config.deposits_paused,
        TradestarsArenaError::DepositsPaused
    );
    verify_deposit_attestation(
        &ctx.accounts.platform_config,
        ctx.program_id,
        &ctx.accounts.user.key(),
        amount,
        &base_tx_hash,
        log_index,
        &signature,
        recovery_id,
    )?;

    let user_account = &mut ctx.accounts.user_account;
    if user_account.owner == Pubkey::default() {
        user_account.owner = ctx.accounts.user.key();
        user_account.total_balance = 0;
        user_account.in_play_debt = 0;
        user_account.last_nonce = 0;
        user_account.bump = ctx.bumps.user_account;
    }

    require!(
        user_account.owner == ctx.accounts.user.key(),
        TradestarsArenaError::InvalidUserAccount
    );

    user_account.bump = ctx.bumps.user_account;
    user_account.total_balance = user_account
        .total_balance
        .checked_add(amount)
        .ok_or(TradestarsArenaError::MathOverflow)?;

    ctx.accounts.deposit_marker.bump = ctx.bumps.deposit_marker;

    mint_to_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            MintToChecked {
                mint: ctx.accounts.tusdc_mint.to_account_info(),
                to: ctx.accounts.user_tusdc.to_account_info(),
                authority: ctx.accounts.platform_config.to_account_info(),
            },
            &[&[CONFIG_SEED, &[ctx.accounts.platform_config.bump]]],
        ),
        amount,
        TUSDC_DECIMALS,
    )?;

    emit!(CollateralDeposited {
        user: ctx.accounts.user.key(),
        amount,
        base_tx_hash,
        log_index,
        new_total_balance: user_account.total_balance,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}
