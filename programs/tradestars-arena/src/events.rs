use anchor_lang::prelude::*;

#[event]
pub struct CollateralDeposited {
    pub user: Pubkey,
    pub amount: u64,
    pub base_tx_hash: [u8; 32],
    pub log_index: u32,
    pub new_total_balance: u64,
    pub timestamp: i64,
}

#[event]
pub struct ArenaCreated {
    pub arena: Pubkey,
    pub creator: Pubkey,
    pub entry_fee: u64,
    pub fee_bps: u16,
    pub guaranteed_prize_target: u64,
    pub guaranteed_prize_reserved: u64,
    pub start_time: i64,
    pub entry_close_time: i64,
    pub end_time: i64,
    pub metadata_hash: [u8; 32],
    pub timestamp: i64,
}

#[event]
pub struct ArenaJoined {
    pub arena: Pubkey,
    pub user: Pubkey,
    pub entry_fee: u64,
    pub fee_amount: u64,
    pub net_entry_amount: u64,
    pub entry_count: u8,
    pub locked_amount: u64,
    pub user_in_play_debt: u64,
    pub total_pool: u64,
    pub timestamp: i64,
}

#[event]
pub struct SettlementRootPosted {
    pub arena: Pubkey,
    pub settlement_version: u32,
    pub merkle_root: [u8; 32],
    pub settlement_timestamp: i64,
    pub claimable_at: i64,
}

#[event]
pub struct ArenaDisputed {
    pub arena: Pubkey,
    pub user: Pubkey,
    pub settlement_version: u32,
    pub dispute_count: u32,
    pub participant_count: u32,
    pub threshold_bps: u16,
    pub disputed: bool,
    pub timestamp: i64,
}

#[event]
pub struct ArenaPositionSettled {
    pub arena: Pubkey,
    pub user: Pubkey,
    pub settled_by: Pubkey,
    pub locked_amount: u64,
    pub payout_amount: u64,
    pub new_total_balance: u64,
    pub new_in_play_debt: u64,
    pub total_claimed_payout: u64,
    pub timestamp: i64,
}

#[event]
pub struct ArenaRefunded {
    pub arena: Pubkey,
    pub user: Pubkey,
    pub refunded_by: Pubkey,
    pub unlocked_amount: u64,
    pub new_in_play_debt: u64,
    pub timestamp: i64,
}

#[event]
pub struct ArenaCancelled {
    pub arena: Pubkey,
    pub creator: Pubkey,
    pub cancelled_by: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct DepositsPauseUpdated {
    pub paused: bool,
    pub authority: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct ArenaFinalized {
    pub arena: Pubkey,
    pub creator: Pubkey,
    pub unused_guarantee: u64,
    pub fees_minted: u64,
    pub timestamp: i64,
}

#[event]
pub struct WithdrawRequested {
    pub user: Pubkey,
    pub amount: u64,
    pub nonce: u64,
    pub remaining_total_balance: u64,
    pub available_to_withdraw: u64,
    pub timestamp: i64,
}
