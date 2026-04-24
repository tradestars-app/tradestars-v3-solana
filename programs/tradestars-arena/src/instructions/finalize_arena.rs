use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token_2022::{self, mint_to_checked, MintToChecked};
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

use crate::errors::TradestarsArenaError;
use crate::events::ArenaFinalized;
use crate::state::{ArenaAccount, ArenaStatus, PlatformConfig, UserAccount};
use crate::utils::{
    guaranteed_prize_used, unused_guarantee, ARENA_SEED, CONFIG_SEED, TUSDC_DECIMALS,
    TUSDC_MINT_SEED, USER_SEED,
};

#[derive(Accounts)]
pub struct FinalizeArena<'info> {
    #[account(seeds = [CONFIG_SEED], bump = platform_config.bump)]
    pub platform_config: Box<Account<'info, PlatformConfig>>,

    #[account(
        mut,
        seeds = [ARENA_SEED, arena.arena_id.as_ref()],
        bump = arena.bump
    )]
    pub arena: Box<Account<'info, ArenaAccount>>,

    #[account(address = arena.creator)]
    /// CHECK: address is verified against arena.creator.
    pub creator: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [USER_SEED, creator.key().as_ref()],
        bump = creator_account.bump
    )]
    pub creator_account: Box<Account<'info, UserAccount>>,

    #[account(
        init_if_needed,
        payer = payer,
        associated_token::mint = tusdc_mint,
        associated_token::authority = creator,
        associated_token::token_program = token_program
    )]
    pub creator_tusdc: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(address = platform_config.treasury_wallet)]
    /// CHECK: address is verified against platform config.
    pub treasury_wallet: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [USER_SEED, treasury_wallet.key().as_ref()],
        bump = treasury_account.bump
    )]
    pub treasury_account: Box<Account<'info, UserAccount>>,

    #[account(
        init_if_needed,
        payer = payer,
        associated_token::mint = tusdc_mint,
        associated_token::authority = treasury_wallet,
        associated_token::token_program = token_program
    )]
    pub treasury_tusdc: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut, seeds = [TUSDC_MINT_SEED], bump)]
    pub tusdc_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub payer: Signer<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<FinalizeArena>) -> Result<()> {
    require!(
        ctx.accounts.token_program.key() == token_2022::ID,
        TradestarsArenaError::InvalidTokenProgram
    );
    require!(
        matches!(
            ctx.accounts.arena.status,
            ArenaStatus::SettledPendingClaim | ArenaStatus::Cancelled
        ),
        TradestarsArenaError::InvalidStatusTransition
    );
    require!(
        ctx.accounts.arena.resolved_count == ctx.accounts.arena.participant_count,
        TradestarsArenaError::ArenaNotReadyToFinalize
    );
    require!(
        ctx.accounts.creator_account.owner == ctx.accounts.creator.key(),
        TradestarsArenaError::InvalidUserAccount
    );
    require!(
        ctx.accounts.treasury_account.owner == ctx.accounts.treasury_wallet.key(),
        TradestarsArenaError::InvalidUserAccount
    );

    let now = Clock::get()?.unix_timestamp;
    let arena = &mut ctx.accounts.arena;
    let creator_account = &mut ctx.accounts.creator_account;
    let treasury_account = &mut ctx.accounts.treasury_account;

    let (guaranteed_used, unused_guarantee_amount, fees_to_mint) =
        if arena.status == ArenaStatus::Cancelled {
            (0_u64, arena.guaranteed_prize_reserved, 0_u64)
        } else {
            (
                guaranteed_prize_used(arena)?,
                unused_guarantee(arena)?,
                arena.fee_accrued,
            )
        };

    creator_account.in_play_debt = creator_account
        .in_play_debt
        .checked_sub(arena.guaranteed_prize_reserved)
        .ok_or(TradestarsArenaError::MathOverflow)?;

    if guaranteed_used > 0 {
        creator_account.total_balance = creator_account
            .total_balance
            .checked_sub(guaranteed_used)
            .ok_or(TradestarsArenaError::MathOverflow)?;
    }

    if unused_guarantee_amount > 0 {
        mint_to_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                MintToChecked {
                    mint: ctx.accounts.tusdc_mint.to_account_info(),
                    to: ctx.accounts.creator_tusdc.to_account_info(),
                    authority: ctx.accounts.platform_config.to_account_info(),
                },
                &[&[CONFIG_SEED, &[ctx.accounts.platform_config.bump]]],
            ),
            unused_guarantee_amount,
            TUSDC_DECIMALS,
        )?;
    }

    if fees_to_mint > 0 {
        mint_to_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                MintToChecked {
                    mint: ctx.accounts.tusdc_mint.to_account_info(),
                    to: ctx.accounts.treasury_tusdc.to_account_info(),
                    authority: ctx.accounts.platform_config.to_account_info(),
                },
                &[&[CONFIG_SEED, &[ctx.accounts.platform_config.bump]]],
            ),
            fees_to_mint,
            TUSDC_DECIMALS,
        )?;

        treasury_account.total_balance = treasury_account
            .total_balance
            .checked_add(fees_to_mint)
            .ok_or(TradestarsArenaError::MathOverflow)?;
    }

    arena.status = ArenaStatus::Finalized;

    emit!(ArenaFinalized {
        arena: arena.key(),
        creator: arena.creator,
        unused_guarantee: unused_guarantee_amount,
        fees_minted: fees_to_mint,
        timestamp: now,
    });

    Ok(())
}
