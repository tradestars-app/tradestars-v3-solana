use anchor_lang::prelude::*;
use anchor_spl::associated_token::get_associated_token_address_with_program_id;
use anchor_spl::token_2022::{self, mint_to_checked, MintToChecked};
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

use crate::errors::TradestarsArenaError;
use crate::events::ArenaRefunded;
use crate::state::{ArenaAccount, ArenaPosition, ArenaStatus, PlatformConfig, UserAccount};
use crate::utils::{
    apply_refund, require_arena_operator, ARENA_SEED, CONFIG_SEED, POSITION_SEED,
    TUSDC_DECIMALS, TUSDC_MINT_SEED, USER_SEED,
};

#[derive(Accounts)]
pub struct RefundArenaBatch<'info> {
    #[account(seeds = [CONFIG_SEED], bump = platform_config.bump)]
    pub platform_config: Account<'info, PlatformConfig>,

    #[account(
        mut,
        seeds = [ARENA_SEED, arena.arena_id.as_ref()],
        bump = arena.bump
    )]
    pub arena: Account<'info, ArenaAccount>,

    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(mut, seeds = [TUSDC_MINT_SEED], bump)]
    pub tusdc_mint: InterfaceAccount<'info, Mint>,

    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler<'info>(ctx: Context<'_, '_, 'info, 'info, RefundArenaBatch<'info>>) -> Result<()> {
    require!(
        ctx.accounts.token_program.key() == token_2022::ID,
        TradestarsArenaError::InvalidTokenProgram
    );
    require_arena_operator(&ctx.accounts.platform_config, &ctx.accounts.authority.key())?;
    require!(
        ctx.accounts.arena.status == ArenaStatus::Cancelled,
        TradestarsArenaError::ArenaNotCancelled
    );
    require!(
        ctx.remaining_accounts.len() % 3 == 0,
        TradestarsArenaError::InvalidBatchAccounts
    );

    let mint_key = ctx.accounts.tusdc_mint.key();
    let now = Clock::get()?.unix_timestamp;

    for chunk in ctx.remaining_accounts.chunks_exact(3) {
        let mut user_account = Account::<UserAccount>::try_from(&chunk[0])?;
        let user_tusdc = InterfaceAccount::<TokenAccount>::try_from(&chunk[1])?;
        let mut position = Account::<ArenaPosition>::try_from(&chunk[2])?;
        let user_key = user_account.owner;
        let (expected_user_pda, _) =
            Pubkey::find_program_address(&[USER_SEED, user_key.as_ref()], ctx.program_id);
        let (expected_position_pda, _) = Pubkey::find_program_address(
            &[POSITION_SEED, ctx.accounts.arena.key().as_ref(), user_key.as_ref()],
            ctx.program_id,
        );
        let expected_user_tusdc =
            get_associated_token_address_with_program_id(&user_key, &mint_key, &ctx.accounts.token_program.key());

        require!(
            chunk[0].key() == expected_user_pda,
            TradestarsArenaError::InvalidUserAccount
        );
        require!(
            chunk[1].key() == expected_user_tusdc,
            TradestarsArenaError::InvalidTokenAccount
        );
        require!(user_tusdc.owner == user_key, TradestarsArenaError::InvalidTokenAccount);
        require!(user_tusdc.mint == mint_key, TradestarsArenaError::InvalidTokenAccount);
        require!(
            chunk[2].key() == expected_position_pda,
            TradestarsArenaError::InvalidArenaPosition
        );
        require!(
            position.user == user_key && position.arena == ctx.accounts.arena.key(),
            TradestarsArenaError::InvalidArenaPosition
        );

        let refunded_amount = apply_refund(
            &mut ctx.accounts.arena,
            &mut user_account,
            &mut position,
            &user_key,
        )?;

        if refunded_amount > 0 {
            mint_to_checked(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    MintToChecked {
                        mint: ctx.accounts.tusdc_mint.to_account_info(),
                        to: user_tusdc.to_account_info(),
                        authority: ctx.accounts.platform_config.to_account_info(),
                    },
                    &[&[CONFIG_SEED, &[ctx.accounts.platform_config.bump]]],
                ),
                refunded_amount,
                TUSDC_DECIMALS,
            )?;
        }

        emit!(ArenaRefunded {
            arena: ctx.accounts.arena.key(),
            user: user_key,
            refunded_by: ctx.accounts.authority.key(),
            unlocked_amount: refunded_amount,
            new_in_play_debt: user_account.in_play_debt,
            timestamp: now,
        });

        user_account.exit(ctx.program_id)?;
        position.exit(ctx.program_id)?;
    }

    Ok(())
}
