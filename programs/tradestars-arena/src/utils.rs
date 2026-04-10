use anchor_lang::prelude::*;

use crate::errors::TradestarsArenaError;

pub fn calculate_fee(amount: u64, fee_bps: u16) -> Result<u64> {
    let fee = (amount as u128)
        .checked_mul(fee_bps as u128)
        .ok_or(TradestarsArenaError::MathOverflow)?
        .checked_div(10_000)
        .ok_or(TradestarsArenaError::MathOverflow)?;
    Ok(fee as u64)
}
