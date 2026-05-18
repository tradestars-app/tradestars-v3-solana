use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token_2022::{burn_checked, BurnChecked};
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

use crate::errors::TradestarsArenaError;
use crate::events::ArenaCreated;
use crate::state::{ArenaAccount, ArenaStatus, CreateArenaParams, PlatformConfig, UserAccount};
use crate::utils::{
    require_arena_operator, ARENA_SEED, CONFIG_SEED, TUSDC_DECIMALS, TUSDC_MINT_SEED, USER_SEED,
};

#[derive(Accounts)]
#[instruction(arena_id: [u8; 32])]
pub struct CreateArena<'info> {
    #[account(seeds = [CONFIG_SEED], bump = platform_config.bump)]
    pub platform_config: Account<'info, PlatformConfig>,

    #[account(
        init,
        payer = authority,
        space = ArenaAccount::LEN,
        seeds = [ARENA_SEED, arena_id.as_ref()],
        bump
    )]
    pub arena: Account<'info, ArenaAccount>,

    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(mut)]
    pub creator: Signer<'info>,

    #[account(
        init_if_needed,
        payer = authority,
        space = UserAccount::LEN,
        seeds = [USER_SEED, creator.key().as_ref()],
        bump
    )]
    pub creator_user_account: Account<'info, UserAccount>,

    #[account(mut, seeds = [TUSDC_MINT_SEED], bump)]
    pub tusdc_mint: InterfaceAccount<'info, Mint>,

    #[account(
        init_if_needed,
        payer = authority,
        associated_token::mint = tusdc_mint,
        associated_token::authority = creator,
        associated_token::token_program = token_program
    )]
    pub creator_tusdc: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<CreateArena>,
    arena_id: [u8; 32],
    params: CreateArenaParams,
) -> Result<()> {
    require_arena_operator(&ctx.accounts.platform_config, &ctx.accounts.authority.key())?;
    require!(params.creator == ctx.accounts.creator.key(), TradestarsArenaError::InvalidCreator);
    require!(params.entry_fee > 0, TradestarsArenaError::AmountTooSmall);
    require!(params.fee_bps <= 10_000, TradestarsArenaError::InvalidFeeBps);

    let now = Clock::get()?.unix_timestamp;
    require!(params.start_time > now, TradestarsArenaError::InvalidTime);
    require!(
        params.entry_close_time >= params.start_time,
        TradestarsArenaError::InvalidTime
    );
    require!(
        params.end_time > params.entry_close_time,
        TradestarsArenaError::InvalidTime
    );

    let creator_user_account = &mut ctx.accounts.creator_user_account;
    if creator_user_account.owner == Pubkey::default() {
        creator_user_account.owner = ctx.accounts.creator.key();
        creator_user_account.total_balance = 0;
        creator_user_account.in_play_debt = 0;
        creator_user_account.last_nonce = 0;
        creator_user_account.bump = ctx.bumps.creator_user_account;
    }
    require!(
        creator_user_account.owner == ctx.accounts.creator.key(),
        TradestarsArenaError::InvalidUserAccount
    );

    if params.guaranteed_prize_target > 0 {
        require!(
            creator_user_account.available_to_withdraw()? >= params.guaranteed_prize_target,
            TradestarsArenaError::InsufficientAvailableBalance
        );

        burn_checked(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                BurnChecked {
                    mint: ctx.accounts.tusdc_mint.to_account_info(),
                    from: ctx.accounts.creator_tusdc.to_account_info(),
                    authority: ctx.accounts.creator.to_account_info(),
                },
            ),
            params.guaranteed_prize_target,
            TUSDC_DECIMALS,
        )?;

        creator_user_account.in_play_debt = creator_user_account
            .in_play_debt
            .checked_add(params.guaranteed_prize_target)
            .ok_or(TradestarsArenaError::MathOverflow)?;
    }

    ctx.accounts.arena.set_inner(ArenaAccount {
        arena_id,
        creator: params.creator,
        status: ArenaStatus::Created,
        entry_fee: params.entry_fee,
        fee_bps: params.fee_bps,
        guaranteed_prize_target: params.guaranteed_prize_target,
        guaranteed_prize_reserved: params.guaranteed_prize_target,
        total_entry_fees_locked: 0,
        fee_accrued: 0,
        total_pool: params.guaranteed_prize_target,
        total_claimed_payout: 0,
        start_time: params.start_time,
        entry_close_time: params.entry_close_time,
        end_time: params.end_time,
        merkle_root: [0; 32],
        settlement_timestamp: 0,
        claimable_at: 0,
        participant_count: 0,
        resolved_count: 0,
        dispute_count: 0,
        settlement_version: 0,
        metadata_hash: params.metadata_hash,
        bump: ctx.bumps.arena,
    });

    emit!(ArenaCreated {
        arena: ctx.accounts.arena.key(),
        creator: params.creator,
        entry_fee: params.entry_fee,
        fee_bps: params.fee_bps,
        guaranteed_prize_target: params.guaranteed_prize_target,
        guaranteed_prize_reserved: params.guaranteed_prize_target,
        start_time: params.start_time,
        entry_close_time: params.entry_close_time,
        end_time: params.end_time,
        metadata_hash: params.metadata_hash,
        timestamp: now,
    });

    Ok(())
}
