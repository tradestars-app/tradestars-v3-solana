use anchor_lang::prelude::*;

#[error_code]
pub enum TradestarsArenaError {
    #[msg("Platform is paused")]
    PlatformPaused,
    #[msg("Unauthorized action")]
    Unauthorized,
    #[msg("Arena is not open")]
    ArenaNotOpen,
    #[msg("Arena is already settled")]
    ArenaAlreadySettled,
    #[msg("Arena has been cancelled")]
    ArenaCancelled,
    #[msg("Arena is not cancelled")]
    ArenaNotCancelled,
    #[msg("Arena has not ended yet")]
    ArenaNotEnded,
    #[msg("Invalid time parameters")]
    InvalidTime,
    #[msg("Invalid status transition")]
    InvalidStatusTransition,
    #[msg("Entry number exceeds max entries per user")]
    EntryLimitReached,
    #[msg("Invalid entry number")]
    InvalidEntryNumber,
    #[msg("Entry does not belong to user")]
    EntryNotOwned,
    #[msg("Payout total exceeds distributable amount")]
    PayoutExceedsPool,
    #[msg("Entry has already been settled")]
    EntryAlreadySettled,
    #[msg("No payout available")]
    NoPayout,
    #[msg("Insufficient funds in vault")]
    InsufficientFunds,
    #[msg("Amount too small")]
    AmountTooSmall,
    #[msg("Calculation overflow")]
    MathOverflow,
    #[msg("Entry window is closed")]
    EntryWindowClosed,
    #[msg("Invalid platform fee bps")]
    InvalidFeeBps,
    #[msg("Batch arrays have mismatched lengths")]
    BatchLengthMismatch,
    #[msg("Invalid remaining accounts passed for batch settlement")]
    InvalidRemainingAccounts,
    #[msg("Invalid winner token account")]
    InvalidWinnerTokenAccount,
    #[msg("Invalid token account")]
    InvalidTokenAccount,
    #[msg("Invalid arena ID length")]
    InvalidArenaId,
}
