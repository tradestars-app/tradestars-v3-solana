use anchor_lang::prelude::*;

use crate::errors::TradestarsArenaError;
use crate::state::PlatformConfig;
use crate::utils::CONFIG_SEED;

#[derive(Accounts)]
pub struct UpdatePlatformConfig<'info> {
    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = platform_config.bump,
        has_one = authority
    )]
    pub platform_config: Account<'info, PlatformConfig>,

    pub authority: Signer<'info>,
}

pub fn handler(
    ctx: Context<UpdatePlatformConfig>,
    new_authority: Option<Pubkey>,
    new_arena_operator: Option<Pubkey>,
    new_treasury_wallet: Option<Pubkey>,
    new_base_attester_eth_address: Option<[u8; 20]>,
    new_base_chain_id: Option<u64>,
    new_base_contract_address: Option<[u8; 20]>,
    new_dispute_window_seconds: Option<u64>,
    new_settlement_grace_period_seconds: Option<u64>,
) -> Result<()> {
    let config = &mut ctx.accounts.platform_config;

    if let Some(authority) = new_authority {
        require!(authority != Pubkey::default(), TradestarsArenaError::InvalidRoleKey);
        config.authority = authority;
    }
    if let Some(arena_operator) = new_arena_operator {
        require!(
            arena_operator != Pubkey::default(),
            TradestarsArenaError::InvalidRoleKey
        );
        config.arena_operator = arena_operator;
    }
    if let Some(treasury_wallet) = new_treasury_wallet {
        require!(
            treasury_wallet != Pubkey::default(),
            TradestarsArenaError::InvalidTreasuryWallet
        );
        config.treasury_wallet = treasury_wallet;
    }
    if let Some(base_attester_eth_address) = new_base_attester_eth_address {
        require!(
            base_attester_eth_address != [0_u8; 20],
            TradestarsArenaError::InvalidBaseSource
        );
        config.base_attester_eth_address = base_attester_eth_address;
    }
    if let Some(base_chain_id) = new_base_chain_id {
        require!(base_chain_id > 0, TradestarsArenaError::InvalidBaseSource);
        config.base_chain_id = base_chain_id;
    }
    if let Some(base_contract_address) = new_base_contract_address {
        require!(
            base_contract_address != [0_u8; 20],
            TradestarsArenaError::InvalidBaseSource
        );
        config.base_contract_address = base_contract_address;
    }
    if let Some(dispute_window_seconds) = new_dispute_window_seconds {
        require!(dispute_window_seconds > 0, TradestarsArenaError::InvalidCooldown);
        config.dispute_window_seconds = dispute_window_seconds;
    }
    if let Some(settlement_grace_period_seconds) = new_settlement_grace_period_seconds {
        require!(
            settlement_grace_period_seconds > 0,
            TradestarsArenaError::InvalidCooldown
        );
        config.settlement_grace_period_seconds = settlement_grace_period_seconds;
    }

    Ok(())
}
