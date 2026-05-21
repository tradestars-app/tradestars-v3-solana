use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token_2022::{self, mint_to_checked, MintToChecked};
use anchor_spl::token_interface::{
    transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked,
};

use crate::errors::TradestarsArenaError;
use crate::events::ExternalWalletUsdcDeposited;
use crate::state::{Marker, PlatformConfig, UserAccount, WalletDepositConfig};
use crate::utils::{
    CONFIG_SEED, TUSDC_DECIMALS, TUSDC_MINT_SEED, USER_SEED, WALLET_DEPOSIT_CONFIG_SEED,
    WALLET_DEPOSIT_SEED,
};

#[derive(Accounts)]
#[instruction(amount: u64, nonce: u64)]
pub struct DepositExternalWalletUsdc<'info> {
    #[account(seeds = [CONFIG_SEED], bump = platform_config.bump)]
    pub platform_config: Box<Account<'info, PlatformConfig>>,

    #[account(seeds = [WALLET_DEPOSIT_CONFIG_SEED], bump = wallet_deposit_config.bump)]
    pub wallet_deposit_config: Box<Account<'info, WalletDepositConfig>>,

    #[account(mut, seeds = [TUSDC_MINT_SEED], bump)]
    pub tusdc_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        constraint = usdc_mint.key() == wallet_deposit_config.usdc_mint @ TradestarsArenaError::InvalidCollateralMint
    )]
    pub usdc_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub source_authority: Signer<'info>,

    /// CHECK: canonical TradeStars wallet receiving playable USD.
    pub destination_user: UncheckedAccount<'info>,

    #[account(mut)]
    pub fee_payer: Signer<'info>,

    #[account(
        init_if_needed,
        payer = fee_payer,
        space = UserAccount::LEN,
        seeds = [USER_SEED, destination_user.key().as_ref()],
        bump
    )]
    pub user_account: Box<Account<'info, UserAccount>>,

    #[account(
        init,
        payer = fee_payer,
        space = Marker::LEN,
        seeds = [
            WALLET_DEPOSIT_SEED,
            destination_user.key().as_ref(),
            source_authority.key().as_ref(),
            &nonce.to_le_bytes()
        ],
        bump
    )]
    pub wallet_deposit_marker: Box<Account<'info, Marker>>,

    #[account(
        mut,
        associated_token::mint = usdc_mint,
        associated_token::authority = source_authority,
        associated_token::token_program = usdc_token_program
    )]
    pub source_usdc: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = fee_payer,
        associated_token::mint = usdc_mint,
        associated_token::authority = platform_config,
        associated_token::token_program = usdc_token_program
    )]
    pub usdc_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = fee_payer,
        associated_token::mint = tusdc_mint,
        associated_token::authority = destination_user,
        associated_token::token_program = tusdc_token_program
    )]
    pub destination_tusdc: Box<InterfaceAccount<'info, TokenAccount>>,

    pub usdc_token_program: Interface<'info, TokenInterface>,
    pub tusdc_token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<DepositExternalWalletUsdc>, amount: u64, nonce: u64) -> Result<()> {
    require!(amount > 0, TradestarsArenaError::AmountTooSmall);
    require!(
        ctx.accounts.tusdc_token_program.key() == token_2022::ID,
        TradestarsArenaError::InvalidTokenProgram
    );
    require!(
        !ctx.accounts.platform_config.deposits_paused,
        TradestarsArenaError::DepositsPaused
    );
    require!(
        ctx.accounts.usdc_mint.decimals == TUSDC_DECIMALS,
        TradestarsArenaError::InvalidCollateralMint
    );

    let user_account = &mut ctx.accounts.user_account;
    if user_account.owner == Pubkey::default() {
        user_account.owner = ctx.accounts.destination_user.key();
        user_account.total_balance = 0;
        user_account.in_play_debt = 0;
        user_account.last_nonce = 0;
        user_account.bump = ctx.bumps.user_account;
    }

    require!(
        user_account.owner == ctx.accounts.destination_user.key(),
        TradestarsArenaError::InvalidUserAccount
    );

    transfer_checked(
        CpiContext::new(
            ctx.accounts.usdc_token_program.to_account_info(),
            TransferChecked {
                mint: ctx.accounts.usdc_mint.to_account_info(),
                from: ctx.accounts.source_usdc.to_account_info(),
                to: ctx.accounts.usdc_vault.to_account_info(),
                authority: ctx.accounts.source_authority.to_account_info(),
            },
        ),
        amount,
        ctx.accounts.usdc_mint.decimals,
    )?;

    user_account.bump = ctx.bumps.user_account;
    user_account.total_balance = user_account
        .total_balance
        .checked_add(amount)
        .ok_or(TradestarsArenaError::MathOverflow)?;

    ctx.accounts.wallet_deposit_marker.bump = ctx.bumps.wallet_deposit_marker;

    mint_to_checked(
        CpiContext::new_with_signer(
            ctx.accounts.tusdc_token_program.to_account_info(),
            MintToChecked {
                mint: ctx.accounts.tusdc_mint.to_account_info(),
                to: ctx.accounts.destination_tusdc.to_account_info(),
                authority: ctx.accounts.platform_config.to_account_info(),
            },
            &[&[CONFIG_SEED, &[ctx.accounts.platform_config.bump]]],
        ),
        amount,
        TUSDC_DECIMALS,
    )?;

    emit!(ExternalWalletUsdcDeposited {
        source_authority: ctx.accounts.source_authority.key(),
        destination_user: ctx.accounts.destination_user.key(),
        amount,
        nonce,
        usdc_mint: ctx.accounts.usdc_mint.key(),
        usdc_vault: ctx.accounts.usdc_vault.key(),
        new_total_balance: user_account.total_balance,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}
