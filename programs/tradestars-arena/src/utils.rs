use anchor_lang::prelude::*;
use anchor_lang::solana_program::{program::invoke_signed, system_instruction, system_program};
use solana_keccak_hasher::hashv;
use spl_token_2022::extension::ExtensionType;

use crate::errors::TradestarsArenaError;
use crate::state::{
    ArenaAccount, ArenaPosition, ArenaStatus, PlatformConfig, UserAccount, DISPUTE_MIN_PARTICIPANTS,
    DISPUTE_THRESHOLD_BPS,
};

pub const CONFIG_SEED: &[u8] = b"config";
pub const USER_SEED: &[u8] = b"user";
pub const ARENA_SEED: &[u8] = b"arena";
pub const POSITION_SEED: &[u8] = b"position";
pub const DEPOSIT_SEED: &[u8] = b"deposit";
pub const TUSDC_MINT_SEED: &[u8] = b"tusdc_mint";
pub const TUSDC_DECIMALS: u8 = 6;

pub fn require_authority(config: &PlatformConfig, signer: &Pubkey) -> Result<()> {
    require!(signer == &config.authority, TradestarsArenaError::Unauthorized);
    Ok(())
}

pub fn require_arena_operator(config: &PlatformConfig, signer: &Pubkey) -> Result<()> {
    require!(
        signer == &config.arena_operator,
        TradestarsArenaError::InvalidArenaOperator
    );
    Ok(())
}

pub fn require_minting_authority(config: &PlatformConfig, signer: &Pubkey) -> Result<()> {
    require!(
        signer == &config.minting_authority,
        TradestarsArenaError::InvalidMintingAuthority
    );
    Ok(())
}

pub fn create_pda_account<'info>(
    payer: &AccountInfo<'info>,
    account: &AccountInfo<'info>,
    system_program_info: &AccountInfo<'info>,
    owner: &Pubkey,
    space: usize,
    signer_seeds: &[&[u8]],
) -> Result<()> {
    require!(
        account.owner == &system_program::ID && account.lamports() == 0,
        TradestarsArenaError::InvalidAccount
    );

    let rent = Rent::get()?;
    let lamports = rent.minimum_balance(space);
    let instruction = system_instruction::create_account(
        payer.key,
        account.key,
        lamports,
        space as u64,
        owner,
    );

    invoke_signed(
        &instruction,
        &[payer.clone(), account.clone(), system_program_info.clone()],
        &[signer_seeds],
    )?;

    Ok(())
}

pub fn tusdc_mint_space() -> Result<usize> {
    ExtensionType::try_calculate_account_len::<spl_token_2022::state::Mint>(&[
        ExtensionType::NonTransferable,
    ])
    .map_err(Into::into)
}

pub fn verify_merkle_proof(leaf: [u8; 32], proof: &[[u8; 32]], root: [u8; 32]) -> bool {
    let mut computed = leaf;
    for node in proof {
        let (left, right) = if computed <= *node {
            (computed, *node)
        } else {
            (*node, computed)
        };
        computed = hashv(&[&left, &right]).0;
    }
    computed == root
}

pub fn merkle_leaf(
    arena_id: &[u8; 32],
    settlement_version: u32,
    user: &Pubkey,
    locked_amount: u64,
    payout_amount: u64,
) -> [u8; 32] {
    hashv(&[
        arena_id,
        &settlement_version.to_le_bytes(),
        user.as_ref(),
        &locked_amount.to_le_bytes(),
        &payout_amount.to_le_bytes(),
    ])
    .0
}

pub fn fee_amount(entry_fee: u64, fee_bps: u16) -> Result<u64> {
    entry_fee
        .checked_mul(fee_bps as u64)
        .ok_or(TradestarsArenaError::MathOverflow)?
        .checked_div(10_000)
        .ok_or(TradestarsArenaError::MathOverflow.into())
}

pub fn dispute_threshold_reached(dispute_count: u32, participant_count: u32) -> Result<bool> {
    if participant_count == 0 {
        return Ok(false);
    }

    let percentage_threshold = (participant_count as u128)
        .checked_mul(DISPUTE_THRESHOLD_BPS as u128)
        .ok_or(TradestarsArenaError::MathOverflow)?
        .checked_add(9_999)
        .ok_or(TradestarsArenaError::MathOverflow)?
        .checked_div(10_000)
        .ok_or(TradestarsArenaError::MathOverflow)? as u32;
    let required_disputes = percentage_threshold.max(participant_count.min(DISPUTE_MIN_PARTICIPANTS));

    Ok(dispute_count >= required_disputes)
}

pub fn net_entry_pool(arena: &ArenaAccount) -> Result<u64> {
    arena
        .total_entry_fees_locked
        .checked_sub(arena.fee_accrued)
        .ok_or(TradestarsArenaError::MathOverflow.into())
}

pub fn guaranteed_prize_used(arena: &ArenaAccount) -> Result<u64> {
    let entry_pool = net_entry_pool(arena)?;
    Ok(arena.total_claimed_payout.saturating_sub(entry_pool))
}

pub fn unused_guarantee(arena: &ArenaAccount) -> Result<u64> {
    arena
        .guaranteed_prize_reserved
        .checked_sub(guaranteed_prize_used(arena)?)
        .ok_or(TradestarsArenaError::MathOverflow.into())
}

pub fn ensure_settlement_claimable(arena: &ArenaAccount, now: i64) -> Result<()> {
    require!(
        arena.status == ArenaStatus::SettledPendingClaim,
        TradestarsArenaError::ArenaNotSettled
    );
    require!(now >= arena.claimable_at, TradestarsArenaError::ClaimCooldownActive);
    Ok(())
}

pub fn apply_settlement(
    arena: &mut ArenaAccount,
    user_account: &mut UserAccount,
    position: &mut ArenaPosition,
    user: &Pubkey,
    locked_amount: u64,
    payout_amount: u64,
) -> Result<()> {
    require!(
        user_account.owner == *user,
        TradestarsArenaError::InvalidUserAccount
    );
    require!(
        position.user == *user,
        TradestarsArenaError::InvalidArenaPosition
    );
    require!(!position.resolved, TradestarsArenaError::PositionAlreadyResolved);
    require!(
        position.locked_amount == locked_amount,
        TradestarsArenaError::LockedAmountMismatch
    );

    let next_total_claimed = arena
        .total_claimed_payout
        .checked_add(payout_amount)
        .ok_or(TradestarsArenaError::MathOverflow)?;
    require!(
        next_total_claimed <= arena.total_pool,
        TradestarsArenaError::ArenaPoolExceeded
    );

    user_account.in_play_debt = user_account
        .in_play_debt
        .checked_sub(locked_amount)
        .ok_or(TradestarsArenaError::MathOverflow)?;
    user_account.total_balance = user_account
        .total_balance
        .checked_sub(locked_amount)
        .ok_or(TradestarsArenaError::MathOverflow)?
        .checked_add(payout_amount)
        .ok_or(TradestarsArenaError::MathOverflow)?;

    position.resolved = true;
    arena.total_claimed_payout = next_total_claimed;
    arena.resolved_count = arena
        .resolved_count
        .checked_add(1)
        .ok_or(TradestarsArenaError::MathOverflow)?;

    Ok(())
}

pub fn apply_refund(
    arena: &mut ArenaAccount,
    user_account: &mut UserAccount,
    position: &mut ArenaPosition,
    user: &Pubkey,
) -> Result<u64> {
    require!(
        user_account.owner == *user,
        TradestarsArenaError::InvalidUserAccount
    );
    require!(
        position.user == *user,
        TradestarsArenaError::InvalidArenaPosition
    );
    require!(!position.resolved, TradestarsArenaError::PositionAlreadyResolved);

    let locked_amount = position.locked_amount;
    user_account.in_play_debt = user_account
        .in_play_debt
        .checked_sub(locked_amount)
        .ok_or(TradestarsArenaError::MathOverflow)?;
    position.resolved = true;
    arena.resolved_count = arena
        .resolved_count
        .checked_add(1)
        .ok_or(TradestarsArenaError::MathOverflow)?;

    Ok(locked_amount)
}
