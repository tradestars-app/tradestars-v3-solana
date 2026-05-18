use anchor_lang::prelude::*;

pub const MAX_ENTRIES_PER_USER: u8 = 10;
pub const DISPUTE_THRESHOLD_BPS: u16 = 2_000;
pub const DISPUTE_MIN_PARTICIPANTS: u32 = 3;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum ArenaStatus {
    Created,
    SettledPendingClaim,
    Disputed,
    Finalized,
    Cancelled,
}

#[account]
pub struct PlatformConfig {
    pub authority: Pubkey,
    pub arena_operator: Pubkey,
    pub treasury_wallet: Pubkey,
    pub minting_authority: Pubkey,
    pub dispute_window_seconds: u64,
    pub settlement_grace_period_seconds: u64,
    pub deposits_paused: bool,
    pub bump: u8,
}

impl PlatformConfig {
    pub const LEN: usize = 8 + (32 * 4) + 8 + 8 + 1 + 1;
}

#[account]
pub struct UserAccount {
    pub owner: Pubkey,
    pub total_balance: u64,
    pub in_play_debt: u64,
    pub last_nonce: u64,
    pub bump: u8,
}

impl UserAccount {
    pub const LEN: usize = 8 + 32 + 8 + 8 + 8 + 1;

    pub fn available_to_withdraw(&self) -> Result<u64> {
        self.total_balance
            .checked_sub(self.in_play_debt)
            .ok_or(crate::errors::TradestarsArenaError::MathOverflow.into())
    }
}

#[account]
pub struct ArenaAccount {
    pub arena_id: [u8; 32],
    pub creator: Pubkey,
    pub status: ArenaStatus,
    pub entry_fee: u64,
    pub fee_bps: u16,
    pub guaranteed_prize_target: u64,
    pub guaranteed_prize_reserved: u64,
    pub total_entry_fees_locked: u64,
    pub fee_accrued: u64,
    pub total_pool: u64,
    pub total_claimed_payout: u64,
    pub start_time: i64,
    pub entry_close_time: i64,
    pub end_time: i64,
    pub merkle_root: [u8; 32],
    pub settlement_timestamp: i64,
    pub claimable_at: i64,
    pub participant_count: u32,
    pub resolved_count: u32,
    pub dispute_count: u32,
    pub settlement_version: u32,
    pub metadata_hash: [u8; 32],
    pub bump: u8,
}

impl ArenaAccount {
    pub const LEN: usize = 8
        + 32
        + 32
        + 1
        + 8
        + 2
        + (8 * 6)
        + (8 * 5)
        + 32
        + (4 * 4)
        + 32
        + 1;
}

#[account]
pub struct ArenaPosition {
    pub user: Pubkey,
    pub arena: Pubkey,
    pub entry_count: u8,
    pub locked_amount: u64,
    pub resolved: bool,
    pub last_disputed_settlement_version: u32,
    pub bump: u8,
}

impl ArenaPosition {
    pub const LEN: usize = 8 + 32 + 32 + 1 + 8 + 1 + 4 + 1;
}

#[account]
pub struct Marker {
    pub bump: u8,
}

impl Marker {
    pub const LEN: usize = 8 + 1;
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct CreateArenaParams {
    pub creator: Pubkey,
    pub entry_fee: u64,
    pub fee_bps: u16,
    pub guaranteed_prize_target: u64,
    pub start_time: i64,
    pub entry_close_time: i64,
    pub end_time: i64,
    pub metadata_hash: [u8; 32],
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct SettlementEntry {
    pub locked_amount: u64,
    pub payout_amount: u64,
    pub proof: Vec<[u8; 32]>,
}
