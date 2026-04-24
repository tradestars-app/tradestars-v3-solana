use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token_2022::{self, mint_to_checked, MintToChecked};
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

use crate::errors::TradestarsArenaError;
use crate::events::ArenaPositionSettled;
use crate::state::{ArenaAccount, ArenaPosition, PlatformConfig, UserAccount};
use crate::utils::{
    apply_settlement, ensure_settlement_claimable, merkle_leaf, verify_merkle_proof, ARENA_SEED,
    CONFIG_SEED, POSITION_SEED, TUSDC_DECIMALS, TUSDC_MINT_SEED, USER_SEED,
};

#[derive(Accounts)]
pub struct ClaimWinnings<'info> {
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

    #[account(mut)]
    pub user: Signer<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

pub fn handler(
    ctx: Context<ClaimWinnings>,
    locked_amount: u64,
    payout_amount: u64,
    proof: Vec<[u8; 32]>,
) -> Result<()> {
    require!(
        ctx.accounts.token_program.key() == token_2022::ID,
        TradestarsArenaError::InvalidTokenProgram
    );

    let now = Clock::get()?.unix_timestamp;
    let arena_id = ctx.accounts.arena.arena_id;
    let settlement_version = ctx.accounts.arena.settlement_version;
    ensure_settlement_claimable(&ctx.accounts.arena, now)?;

    let leaf = merkle_leaf(
        &arena_id,
        settlement_version,
        &ctx.accounts.user.key(),
        locked_amount,
        payout_amount,
    );
    require!(
        verify_merkle_proof(leaf, &proof, ctx.accounts.arena.merkle_root),
        TradestarsArenaError::InvalidMerkleProof
    );

    apply_settlement(
        &mut ctx.accounts.arena,
        &mut ctx.accounts.user_account,
        &mut ctx.accounts.position,
        &ctx.accounts.user.key(),
        locked_amount,
        payout_amount,
    )?;

    if payout_amount > 0 {
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
            payout_amount,
            TUSDC_DECIMALS,
        )?;
    }

    emit!(ArenaPositionSettled {
        arena: ctx.accounts.arena.key(),
        user: ctx.accounts.user.key(),
        settled_by: ctx.accounts.user.key(),
        locked_amount,
        payout_amount,
        new_total_balance: ctx.accounts.user_account.total_balance,
        new_in_play_debt: ctx.accounts.user_account.in_play_debt,
        total_claimed_payout: ctx.accounts.arena.total_claimed_payout,
        timestamp: now,
    });

    Ok(())
}
