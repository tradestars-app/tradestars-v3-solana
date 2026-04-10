use anchor_lang::prelude::*;
use anchor_lang::solana_program::account_info::next_account_info;
use anchor_spl::token::{transfer, Token, TokenAccount, Transfer};

use crate::errors::TradestarsArenaError;
use crate::state::{Arena, ArenaEntry, ArenaStatus, PlatformConfig};
use crate::utils::calculate_fee;
use crate::{ArenaFinalizedEvent, ArenaSettledEvent, OverlayFundedEvent, WinnerPaidEvent};

#[derive(Accounts)]
pub struct SettleAndPayBatch<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        seeds = [b"config"],
        bump = platform_config.bump,
        has_one = authority
    )]
    pub platform_config: Account<'info, PlatformConfig>,

    #[account(
        mut,
        constraint = arena.authority == authority.key() @ TradestarsArenaError::Unauthorized
    )]
    pub arena: Account<'info, Arena>,

    #[account(
        mut,
        seeds = [b"vault", arena.key().as_ref()],
        bump,
        constraint = arena_vault.owner == arena.key()
    )]
    pub arena_vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [b"platform_vault"],
        bump,
        constraint = platform_vault.owner == platform_config.key(),
        constraint = platform_vault.mint == arena_vault.mint
    )]
    pub platform_vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = source_overlay_usdc.owner == authority.key(),
        constraint = source_overlay_usdc.mint == arena_vault.mint
    )]
    pub source_overlay_usdc: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

pub fn handler<'info>(
    ctx: Context<'_, '_, 'info, 'info, SettleAndPayBatch<'info>>,
    ranks: Vec<u16>,
    payout_amounts: Vec<u64>,
) -> Result<()> {
    let platform_config = &ctx.accounts.platform_config;
    require!(
        !platform_config.paused,
        TradestarsArenaError::PlatformPaused
    );

    require!(
        ranks.len() == payout_amounts.len(),
        TradestarsArenaError::BatchLengthMismatch
    );

    let remaining_len_expected = ranks
        .len()
        .checked_mul(2)
        .ok_or(TradestarsArenaError::MathOverflow)?;
    require!(
        ctx.remaining_accounts.len() == remaining_len_expected,
        TradestarsArenaError::InvalidRemainingAccounts
    );

    match ctx.accounts.arena.status {
        ArenaStatus::Cancelled => return err!(TradestarsArenaError::ArenaCancelled),
        ArenaStatus::Settled => return Ok(()),
        ArenaStatus::Open | ArenaStatus::Finalized => {}
    }

    if ctx.accounts.arena.status == ArenaStatus::Open {
        let now = Clock::get()?.unix_timestamp;
        require!(
            now >= ctx.accounts.arena.end_time,
            TradestarsArenaError::ArenaNotEnded
        );
    }

    let arena_id = ctx.accounts.arena.arena_id.clone();
    let arena_bump = ctx.accounts.arena.bump;
    let arena_key = ctx.accounts.arena.key();
    let signer_seeds = &[b"arena".as_ref(), arena_id.as_bytes(), &[arena_bump]];
    let signer = &[&signer_seeds[..]];

    let mut expected_vault_balance = ctx.accounts.arena_vault.amount;

    if !ctx.accounts.arena.fee_collected {
        let fee = calculate_fee(
            ctx.accounts.arena.total_pool,
            ctx.accounts.platform_config.platform_fee_bps,
        )?;

        if fee > 0 {
            require!(
                expected_vault_balance >= fee,
                TradestarsArenaError::InsufficientFunds
            );

            transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.arena_vault.to_account_info(),
                        to: ctx.accounts.platform_vault.to_account_info(),
                        authority: ctx.accounts.arena.to_account_info(),
                    },
                    signer,
                ),
                fee,
            )?;

            expected_vault_balance = expected_vault_balance
                .checked_sub(fee)
                .ok_or(TradestarsArenaError::MathOverflow)?;
        }

        let collected_net = ctx
            .accounts
            .arena
            .total_pool
            .checked_sub(fee)
            .ok_or(TradestarsArenaError::MathOverflow)?;

        {
            let arena = &mut ctx.accounts.arena;
            arena.fee_amount = fee;
            arena.fee_collected = true;
            arena.status = ArenaStatus::Finalized;
        }

        emit!(ArenaFinalizedEvent {
            arena: arena_key,
            total_entries: ctx.accounts.arena.total_entries,
            total_pool: ctx.accounts.arena.total_pool,
            fee_amount: fee,
            collected_net,
            timestamp: Clock::get()?.unix_timestamp,
        });
    }

    let required_overlay = ctx.accounts.arena.required_overlay()?;
    if ctx.accounts.arena.overlay_funded < required_overlay {
        let target_overlay_total = required_overlay;
        let delta = target_overlay_total
            .checked_sub(ctx.accounts.arena.overlay_funded)
            .ok_or(TradestarsArenaError::MathOverflow)?;

        transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.source_overlay_usdc.to_account_info(),
                    to: ctx.accounts.arena_vault.to_account_info(),
                    authority: ctx.accounts.authority.to_account_info(),
                },
            ),
            delta,
        )?;

        expected_vault_balance = expected_vault_balance
            .checked_add(delta)
            .ok_or(TradestarsArenaError::MathOverflow)?;

        {
            let arena = &mut ctx.accounts.arena;
            arena.overlay_funded = target_overlay_total;
        }

        emit!(OverlayFundedEvent {
            arena: arena_key,
            delta_amount: delta,
            target_overlay_total,
            new_overlay_funded_total: ctx.accounts.arena.overlay_funded,
            timestamp: Clock::get()?.unix_timestamp,
        });
    }

    let max_distributable = ctx.accounts.arena.max_distributable()?;
    let mut running_total = ctx.accounts.arena.total_payouts_set;
    let mut remaining_accounts_iter = ctx.remaining_accounts.iter();

    for idx in 0..ranks.len() {
        let entry_info = next_account_info(&mut remaining_accounts_iter)?;
        let winner_usdc_info = next_account_info(&mut remaining_accounts_iter)?;

        require!(
            entry_info.is_writable,
            TradestarsArenaError::InvalidRemainingAccounts
        );

        let mut entry = Account::<ArenaEntry>::try_from(entry_info)?;
        require!(
            entry.arena == arena_key,
            TradestarsArenaError::InvalidRemainingAccounts
        );

        let winner_usdc = Account::<TokenAccount>::try_from(winner_usdc_info)?;
        require!(
            winner_usdc.owner == entry.user && winner_usdc.mint == ctx.accounts.arena_vault.mint,
            TradestarsArenaError::InvalidWinnerTokenAccount
        );

        let rank = ranks[idx];
        let payout_amount = payout_amounts[idx];

        if entry.settled {
            require!(
                entry.payout_amount == payout_amount,
                TradestarsArenaError::EntryAlreadySettled
            );
            entry.exit(ctx.program_id)?;
            continue;
        }

        require!(rank > 0, TradestarsArenaError::InvalidEntryNumber);
        require!(payout_amount > 0, TradestarsArenaError::NoPayout);

        running_total = running_total
            .checked_add(payout_amount)
            .ok_or(TradestarsArenaError::MathOverflow)?;
        require!(
            running_total <= max_distributable,
            TradestarsArenaError::PayoutExceedsPool
        );
        require!(
            expected_vault_balance >= payout_amount,
            TradestarsArenaError::InsufficientFunds
        );

        transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.arena_vault.to_account_info(),
                    to: winner_usdc_info.to_account_info(),
                    authority: ctx.accounts.arena.to_account_info(),
                },
                signer,
            ),
            payout_amount,
        )?;

        expected_vault_balance = expected_vault_balance
            .checked_sub(payout_amount)
            .ok_or(TradestarsArenaError::MathOverflow)?;

        entry.payout_amount = payout_amount;
        entry.settled = true;

        let entry_key = entry.key();
        let entry_user = entry.user;
        entry.exit(ctx.program_id)?;

        emit!(WinnerPaidEvent {
            arena: arena_key,
            entry: entry_key,
            user: entry_user,
            rank,
            payout_amount,
            total_payouts_set: running_total,
            timestamp: Clock::get()?.unix_timestamp,
        });
    }

    let mut emit_settled = false;
    {
        let arena = &mut ctx.accounts.arena;
        arena.total_payouts_set = running_total;

        if arena.total_payouts_set >= max_distributable {
            arena.status = ArenaStatus::Settled;
            emit_settled = true;
        }
    }

    if emit_settled {
        emit!(ArenaSettledEvent {
            arena: arena_key,
            total_payouts_set: ctx.accounts.arena.total_payouts_set,
            overlay_funded: ctx.accounts.arena.overlay_funded,
            timestamp: Clock::get()?.unix_timestamp,
        });
    }

    Ok(())
}
