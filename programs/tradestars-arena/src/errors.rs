use anchor_lang::prelude::*;

#[error_code]
pub enum TradestarsArenaError {
    #[msg("Unauthorized action")]
    Unauthorized,
    #[msg("Only the configured arena operator may call this instruction")]
    InvalidArenaOperator,
    #[msg("Invalid deposit attestation")]
    InvalidDepositAttestation,
    #[msg("Invalid base source configuration")]
    InvalidBaseSource,
    #[msg("Invalid status transition")]
    InvalidStatusTransition,
    #[msg("Amount too small")]
    AmountTooSmall,
    #[msg("Calculation overflow")]
    MathOverflow,
    #[msg("Invalid arena fee bps")]
    InvalidFeeBps,
    #[msg("Invalid cooldown configuration")]
    InvalidCooldown,
    #[msg("Invalid time configuration")]
    InvalidTime,
    #[msg("Invalid account supplied")]
    InvalidAccount,
    #[msg("User account owner mismatch")]
    InvalidUserAccount,
    #[msg("Arena creator signer mismatch")]
    InvalidCreator,
    #[msg("Invalid treasury wallet")]
    InvalidTreasuryWallet,
    #[msg("Invalid role key")]
    InvalidRoleKey,
    #[msg("Arena position does not match expected PDA")]
    InvalidArenaPosition,
    #[msg("Arena position has already been resolved")]
    PositionAlreadyResolved,
    #[msg("Arena has not been settled")]
    ArenaNotSettled,
    #[msg("Arena is not disputed")]
    ArenaNotDisputed,
    #[msg("Arena is not cancelled")]
    ArenaNotCancelled,
    #[msg("Claim cooldown has not elapsed")]
    ClaimCooldownActive,
    #[msg("Arena claim amount does not match locked amount")]
    LockedAmountMismatch,
    #[msg("Settlement batch accounts are invalid")]
    InvalidBatchAccounts,
    #[msg("Invalid merkle proof")]
    InvalidMerkleProof,
    #[msg("Insufficient unlocked balance")]
    InsufficientAvailableBalance,
    #[msg("Insufficient guaranteed prize reserves")]
    InsufficientGuaranteedPrize,
    #[msg("User has reached the maximum number of entries")]
    MaxEntriesReached,
    #[msg("Arena claim payout exceeds the remaining arena pool")]
    ArenaPoolExceeded,
    #[msg("Claims have already started for this settlement version")]
    ClaimsAlreadyStarted,
    #[msg("Invalid nonce")]
    InvalidNonce,
    #[msg("Expected the Token-2022 program")]
    InvalidTokenProgram,
    #[msg("Invalid token account")]
    InvalidTokenAccount,
    #[msg("User is not an active participant in the arena")]
    NotArenaParticipant,
    #[msg("Arena is not in the created state")]
    ArenaNotCreated,
    #[msg("Arena has unresolved positions")]
    ArenaNotReadyToFinalize,
    #[msg("Arena has already been disputed for this settlement round")]
    AlreadyDisputed,
    #[msg("Deposits are currently paused")]
    DepositsPaused,
    #[msg("Arena is not stale enough for cancellation")]
    ArenaNotStale,
}
