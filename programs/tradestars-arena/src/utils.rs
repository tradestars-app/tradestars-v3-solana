use anchor_lang::prelude::*;
use anchor_lang::solana_program::{program::invoke_signed, system_instruction, system_program};
use solana_keccak_hasher::hashv;
use solana_secp256k1_recover::secp256k1_recover;
use spl_token_2022::extension::ExtensionType;

use crate::errors::TradestarsArenaError;
use crate::state::{
    ArenaAccount, ArenaPosition, ArenaStatus, PlatformConfig, UserAccount, DISPUTE_THRESHOLD_BPS,
};

pub const CONFIG_SEED: &[u8] = b"config";
pub const USER_SEED: &[u8] = b"user";
pub const ARENA_SEED: &[u8] = b"arena";
pub const POSITION_SEED: &[u8] = b"position";
pub const DEPOSIT_SEED: &[u8] = b"deposit";
pub const TUSDC_MINT_SEED: &[u8] = b"tusdc_mint";
pub const TUSDC_DECIMALS: u8 = 6;
pub const DEPOSIT_ATTESTATION_DOMAIN: &[u8] = b"TRADESTARS_BASE_DEPOSIT_V1";

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

pub fn deposit_attestation_hash(
    program_id: &Pubkey,
    chain_id: u64,
    contract_address: &[u8; 20],
    user: &Pubkey,
    amount: u64,
    base_tx_hash: &[u8; 32],
    log_index: u32,
) -> [u8; 32] {
    hashv(&[
        DEPOSIT_ATTESTATION_DOMAIN,
        program_id.as_ref(),
        &chain_id.to_le_bytes(),
        contract_address,
        user.as_ref(),
        &amount.to_le_bytes(),
        base_tx_hash,
        &log_index.to_le_bytes(),
    ])
    .0
}

pub fn ethereum_address_from_pubkey(pubkey: &[u8; 64]) -> [u8; 20] {
    let hash = hashv(&[pubkey]).0;
    let mut address = [0_u8; 20];
    address.copy_from_slice(&hash[12..]);
    address
}

pub fn verify_deposit_attestation(
    config: &PlatformConfig,
    program_id: &Pubkey,
    user: &Pubkey,
    amount: u64,
    base_tx_hash: &[u8; 32],
    log_index: u32,
    signature: &[u8; 64],
    recovery_id: u8,
) -> Result<()> {
    let digest = deposit_attestation_hash(
        program_id,
        config.base_chain_id,
        &config.base_contract_address,
        user,
        amount,
        base_tx_hash,
        log_index,
    );

    let recovered = secp256k1_recover(&digest, recovery_id, signature)
        .map_err(|_| TradestarsArenaError::InvalidDepositAttestation)?;
    let recovered_address = ethereum_address_from_pubkey(&recovered.0);
    require!(
        recovered_address == config.base_attester_eth_address,
        TradestarsArenaError::InvalidDepositAttestation
    );
    Ok(())
}

pub fn fee_amount(entry_fee: u64, fee_bps: u16) -> Result<u64> {
    entry_fee
        .checked_mul(fee_bps as u64)
        .ok_or(TradestarsArenaError::MathOverflow)?
        .checked_div(10_000)
        .ok_or(TradestarsArenaError::MathOverflow.into())
}

pub fn dispute_threshold_reached(dispute_count: u32, participant_count: u32) -> Result<bool> {
    Ok((dispute_count as u128)
        .checked_mul(10_000)
        .ok_or(TradestarsArenaError::MathOverflow)?
        >= (participant_count as u128)
            .checked_mul(DISPUTE_THRESHOLD_BPS as u128)
            .ok_or(TradestarsArenaError::MathOverflow)?)
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
