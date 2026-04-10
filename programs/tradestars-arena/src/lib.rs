#![allow(unexpected_cfgs)]

use anchor_lang::prelude::*;

pub mod errors;
pub mod instructions;
pub mod state;
pub mod utils;

use instructions::*;

declare_id!("6W1s28JYjDunfsFFAv7TcWQWJBUxCPpn8WYMinRyppPy");

#[program]
pub mod tradestars_arena {
    use super::*;

    pub fn initialize_platform(
        ctx: Context<InitializePlatform>,
        fee_recipient: Pubkey,
        platform_fee_bps: u16,
    ) -> Result<()> {
        instructions::initialize_platform::handler(ctx, fee_recipient, platform_fee_bps)
    }

    pub fn create_arena(
        ctx: Context<CreateArena>,
        arena_id: String,
        params: CreateArenaParams,
    ) -> Result<()> {
        instructions::create_arena::handler(ctx, arena_id, params)
    }

    pub fn enter_arena(ctx: Context<EnterArena>, entry_number: u8) -> Result<()> {
        instructions::enter_arena::handler(ctx, entry_number)
    }

    pub fn settle_and_pay_batch<'info>(
        ctx: Context<'_, '_, 'info, 'info, SettleAndPayBatch<'info>>,
        ranks: Vec<u16>,
        payout_amounts: Vec<u64>,
    ) -> Result<()> {
        instructions::settle_and_pay_batch::handler(ctx, ranks, payout_amounts)
    }

    pub fn cancel_arena(ctx: Context<CancelArena>, reason: String) -> Result<()> {
        instructions::cancel_arena::handler(ctx, reason)
    }

    pub fn refund_batch<'info>(
        ctx: Context<'_, '_, 'info, 'info, RefundBatch<'info>>,
    ) -> Result<()> {
        instructions::refund_batch::handler(ctx)
    }

    pub fn withdraw_fees(ctx: Context<WithdrawFees>, amount: Option<u64>) -> Result<()> {
        instructions::withdraw_fees::handler(ctx, amount)
    }

    pub fn toggle_pause(ctx: Context<TogglePause>) -> Result<()> {
        instructions::toggle_pause::handler(ctx)
    }
}

#[event]
pub struct ArenaCreatedEvent {
    pub arena: Pubkey,
    pub arena_id: String,
    pub entry_fee: u64,
    pub guaranteed_prize_pool: u64,
    pub start_time: i64,
    pub end_time: i64,
    pub creator: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct EntryEvent {
    pub arena: Pubkey,
    pub entry: Pubkey,
    pub user: Pubkey,
    pub entry_number: u8,
    pub amount_paid: u64,
    pub timestamp: i64,
}

#[event]
pub struct ArenaFinalizedEvent {
    pub arena: Pubkey,
    pub total_entries: u32,
    pub total_pool: u64,
    pub fee_amount: u64,
    pub collected_net: u64,
    pub timestamp: i64,
}

#[event]
pub struct OverlayFundedEvent {
    pub arena: Pubkey,
    pub delta_amount: u64,
    pub target_overlay_total: u64,
    pub new_overlay_funded_total: u64,
    pub timestamp: i64,
}

#[event]
pub struct WinnerPaidEvent {
    pub arena: Pubkey,
    pub entry: Pubkey,
    pub user: Pubkey,
    pub rank: u16,
    pub payout_amount: u64,
    pub total_payouts_set: u64,
    pub timestamp: i64,
}

#[event]
pub struct ArenaSettledEvent {
    pub arena: Pubkey,
    pub total_payouts_set: u64,
    pub overlay_funded: u64,
    pub timestamp: i64,
}

#[event]
pub struct ArenaCancelledEvent {
    pub arena: Pubkey,
    pub reason: String,
    pub timestamp: i64,
}

#[event]
pub struct RefundClaimedEvent {
    pub arena: Pubkey,
    pub entry: Pubkey,
    pub user: Pubkey,
    pub amount: u64,
    pub total_refunds_paid: u64,
    pub timestamp: i64,
}
