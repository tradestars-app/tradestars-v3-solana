use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token_2022::{self, mint_to_checked, MintToChecked};
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

use crate::errors::TradestarsArenaError;
use crate::events::ArenaRefunded;
use crate::state::{ArenaAccount, ArenaPosition, ArenaStatus, PlatformConfig, UserAccount};
use crate::utils::{
    apply_refund, ARENA_SEED, CONFIG_SEED, POSITION_SEED, TUSDC_DECIMALS, TUSDC_MINT_SEED,
    USER_SEED,
};

#[derive(Accounts)]
pub struct ClaimRefund<'info> {
    #[account(seeds = [CONFIG_SEED], bump = platform_config.bump)]
    pub platform_config: Box<Account<'info, PlatformConfig>>,

    #[account(
        mut,
        seeds = [ARENA_SEED, arena.arena_id.as_ref()],
        bump = arena.bump
    )]
    pub arena: Box<Account<'info, ArenaAccount>>,

    #[account(
        mut,
        seeds = [USER_SEED, user.key().as_ref()],
        bump = user_account.bump
    )]
    pub user_account: Box<Account<'info, UserAccount>>,

    #[account(
        mut,
        seeds = [POSITION_SEED, arena.key().as_ref(), user.key().as_ref()],
        bump = position.bump
    )]
    pub position: Box<Account<'info, ArenaPosition>>,

    #[account(mut, seeds = [TUSDC_MINT_SEED], bump)]
    pub tusdc_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        associated_token::mint = tusdc_mint,
        associated_token::authority = user,
        associated_token::token_program = token_program
    )]
    pub user_tusdc: Box<InterfaceAccount<'info, TokenAccount>>,

    pub user: Signer<'info>,
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

pub fn handler(ctx: Context<ClaimRefund>) -> Result<()> {
    require!(
        ctx.accounts.token_program.key() == token_2022::ID,
        TradestarsArenaError::InvalidTokenProgram
    );
    require!(
        ctx.accounts.arena.status == ArenaStatus::Cancelled,
        TradestarsArenaError::ArenaNotCancelled
    );

    let refunded_amount = apply_refund(
        &mut ctx.accounts.arena,
        &mut ctx.accounts.user_account,
        &mut ctx.accounts.position,
        &ctx.accounts.user.key(),
    )?;

    if refunded_amount > 0 {
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
            refunded_amount,
            TUSDC_DECIMALS,
        )?;
    }

    emit!(ArenaRefunded {
        arena: ctx.accounts.arena.key(),
        user: ctx.accounts.user.key(),
        refunded_by: ctx.accounts.user.key(),
        unlocked_amount: refunded_amount,
        new_in_play_debt: ctx.accounts.user_account.in_play_debt,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}
