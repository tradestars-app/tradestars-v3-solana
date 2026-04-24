#![allow(unexpected_cfgs)]

use anchor_lang::prelude::*;

pub mod errors;
pub mod events;
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
        arena_operator: Pubkey,
        treasury_wallet: Pubkey,
        minting_authority: Pubkey,
        dispute_window_seconds: u64,
        settlement_grace_period_seconds: u64,
    ) -> Result<()> {
        instructions::initialize_platform::handler(
            ctx,
            arena_operator,
            treasury_wallet,
            minting_authority,
            dispute_window_seconds,
            settlement_grace_period_seconds,
        )
    }

    pub fn update_platform_config(
        ctx: Context<UpdatePlatformConfig>,
        new_authority: Option<Pubkey>,
        new_arena_operator: Option<Pubkey>,
        new_treasury_wallet: Option<Pubkey>,
        new_minting_authority: Option<Pubkey>,
        new_dispute_window_seconds: Option<u64>,
        new_settlement_grace_period_seconds: Option<u64>,
    ) -> Result<()> {
        instructions::update_platform_config::handler(
            ctx,
            new_authority,
            new_arena_operator,
            new_treasury_wallet,
            new_minting_authority,
            new_dispute_window_seconds,
            new_settlement_grace_period_seconds,
        )
    }

    pub fn deposit_collateral(
        ctx: Context<DepositCollateral>,
        amount: u64,
        base_tx_hash: [u8; 32],
        log_index: u32,
    ) -> Result<()> {
        instructions::deposit_collateral::handler(
            ctx,
            amount,
            base_tx_hash,
            log_index,
        )
    }

    pub fn create_arena(
        ctx: Context<CreateArena>,
        arena_id: [u8; 32],
        params: state::CreateArenaParams,
    ) -> Result<()> {
        instructions::create_arena::handler(ctx, arena_id, params)
    }

    pub fn join_arena(ctx: Context<JoinArena>) -> Result<()> {
        instructions::join_arena::handler(ctx)
    }

    pub fn post_settlement_root(
        ctx: Context<PostSettlementRoot>,
        merkle_root: [u8; 32],
    ) -> Result<()> {
        instructions::post_settlement_root::handler(ctx, merkle_root)
    }

    pub fn submit_dispute(ctx: Context<SubmitDispute>) -> Result<()> {
        instructions::submit_dispute::handler(ctx)
    }

    pub fn cancel_stale_arena(ctx: Context<CancelStaleArena>) -> Result<()> {
        instructions::cancel_stale_arena::handler(ctx)
    }

    pub fn cancel_disputed_arena(ctx: Context<CancelDisputedArena>) -> Result<()> {
        instructions::cancel_disputed_arena::handler(ctx)
    }

    pub fn set_deposits_paused(ctx: Context<SetDepositsPaused>, paused: bool) -> Result<()> {
        instructions::set_deposits_paused::handler(ctx, paused)
    }

    pub fn settle_arena_batch<'info>(
        ctx: Context<'_, '_, 'info, 'info, SettleArenaBatch<'info>>,
        entries: Vec<state::SettlementEntry>,
    ) -> Result<()> {
        instructions::settle_arena_batch::handler(ctx, entries)
    }

    pub fn claim_winnings(
        ctx: Context<ClaimWinnings>,
        locked_amount: u64,
        payout_amount: u64,
        proof: Vec<[u8; 32]>,
    ) -> Result<()> {
        instructions::claim_winnings::handler(
            ctx,
            locked_amount,
            payout_amount,
            proof,
        )
    }

    pub fn cancel_arena(ctx: Context<CancelArena>) -> Result<()> {
        instructions::cancel_arena::handler(ctx)
    }

    pub fn refund_arena_batch<'info>(
        ctx: Context<'_, '_, 'info, 'info, RefundArenaBatch<'info>>,
    ) -> Result<()> {
        instructions::refund_arena_batch::handler(ctx)
    }

    pub fn claim_refund(ctx: Context<ClaimRefund>) -> Result<()> {
        instructions::claim_refund::handler(ctx)
    }

    pub fn finalize_arena(ctx: Context<FinalizeArena>) -> Result<()> {
        instructions::finalize_arena::handler(ctx)
    }

    pub fn withdraw_request(
        ctx: Context<WithdrawRequest>,
        amount: u64,
        nonce: u64,
    ) -> Result<()> {
        instructions::withdraw_request::handler(ctx, amount, nonce)
    }
}
