use anchor_lang::prelude::*;
use crate::constants::*;
use crate::error::ErrorCode;
use crate::events::ExchangeInitialized;
use crate::state::Exchange;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct InitializeExchangeParams {
    pub swap_fee_bps: u64,
    pub perp_open_fee_bps: u64,
    pub perp_close_fee_bps: u64,
    pub lending_fee_bps: u64,
    pub max_leverage: u64,
    pub liquidation_bonus_bps: u64,
    pub max_liquidation_fraction_bps: u64,
}

#[derive(Accounts)]
pub struct InitializeExchange<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        init,
        payer = admin,
        space = Exchange::LEN,
        seeds = [EXCHANGE_SEED],
        bump,
    )]
    pub exchange: Account<'info, Exchange>,

    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<InitializeExchange>,
    params: InitializeExchangeParams,
) -> Result<()> {
    require!(
        params.swap_fee_bps <= MAX_SWAP_FEE_BPS,
        ErrorCode::FeeExceedsMaximum
    );
    require!(
        params.perp_open_fee_bps <= MAX_PERP_FEE_BPS,
        ErrorCode::FeeExceedsMaximum
    );
    require!(
        params.perp_close_fee_bps <= MAX_PERP_FEE_BPS,
        ErrorCode::FeeExceedsMaximum
    );
    require!(
        params.max_leverage >= MIN_LEVERAGE && params.max_leverage <= MAX_LEVERAGE,
        ErrorCode::LeverageOutOfBounds
    );

    let exchange = &mut ctx.accounts.exchange;
    exchange.admin = ctx.accounts.admin.key();
    exchange.bump = ctx.bumps.exchange;
    exchange.swap_fee_bps = params.swap_fee_bps;
    exchange.perp_open_fee_bps = params.perp_open_fee_bps;
    exchange.perp_close_fee_bps = params.perp_close_fee_bps;
    exchange.lending_fee_bps = params.lending_fee_bps;
    exchange.max_leverage = params.max_leverage;
    exchange.liquidation_bonus_bps = params.liquidation_bonus_bps;
    exchange.max_liquidation_fraction_bps = params.max_liquidation_fraction_bps;
    exchange.swap_paused = false;
    exchange.perp_paused = false;
    exchange.lending_paused = false;
    exchange.total_perp_markets = 0;
    exchange.total_lending_pools = 0;
    exchange.total_users = 0;
    exchange._reserved = [0u8; 128];

    let clock = Clock::get()?;
    emit!(ExchangeInitialized {
        admin: exchange.admin,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}
