use anchor_lang::prelude::*;

use crate::errors::TradestarsArenaError;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum ArenaStatus {
    Open,
    Finalized,
    Settled,
    Cancelled,
}

#[account]
pub struct Arena {
    pub arena_id: String,
    pub entry_fee: u64,
    pub guaranteed_prize_pool: u64,
    pub start_time: i64,
    pub end_time: i64,
    pub total_entries: u32,
    pub total_pool: u64,
    pub fee_amount: u64,
    pub fee_collected: bool,
    pub overlay_funded: u64,
    pub total_payouts_set: u64,
    pub total_refunds_paid: u64,
    pub status: ArenaStatus,
    pub max_entries_per_user: u8,
    pub authority: Pubkey,
    pub creator: Pubkey,
    pub bump: u8,
}

impl Arena {
    pub const MAX_ARENA_ID_LEN: usize = 64;

    pub const LEN: usize = 8 + // discriminator
        4 + Self::MAX_ARENA_ID_LEN + // arena_id
        8 + // entry_fee
        8 + // guaranteed_prize_pool
        8 + // start_time
        8 + // end_time
        4 + // total_entries
        8 + // total_pool
        8 + // fee_amount
        1 + // fee_collected
        8 + // overlay_funded
        8 + // total_payouts_set
        8 + // total_refunds_paid
        1 + // status
        1 + // max_entries_per_user
        32 + // authority
        32 + // creator
        1; // bump

    pub fn collected_net(&self) -> Result<u64> {
        self.total_pool
            .checked_sub(self.fee_amount)
            .ok_or(TradestarsArenaError::MathOverflow.into())
    }

    pub fn required_overlay(&self) -> Result<u64> {
        let collected_net = self.collected_net()?;
        Ok(self.guaranteed_prize_pool.saturating_sub(collected_net))
    }

    pub fn max_distributable(&self) -> Result<u64> {
        let collected_net = self.collected_net()?;
        collected_net
            .checked_add(self.overlay_funded)
            .ok_or(TradestarsArenaError::MathOverflow.into())
    }
}

#[account]
pub struct ArenaEntry {
    pub user: Pubkey,
    pub arena: Pubkey,
    pub entry_number: u8,
    pub amount_paid: u64,
    pub payout_amount: u64,
    pub settled: bool,
    pub bump: u8,
}

impl ArenaEntry {
    pub const LEN: usize = 8 + // discriminator
        32 + // user
        32 + // arena
        1 + // entry_number
        8 + // amount_paid
        8 + // payout_amount
        1 + // settled
        1; // bump
}

#[account]
pub struct PlatformConfig {
    pub authority: Pubkey,
    pub fee_recipient: Pubkey,
    pub platform_fee_bps: u16,
    pub paused: bool,
    pub bump: u8,
}

impl PlatformConfig {
    pub const LEN: usize = 8 + // discriminator
        32 + // authority
        32 + // fee_recipient
        2 + // platform_fee_bps
        1 + // paused
        1; // bump
}
