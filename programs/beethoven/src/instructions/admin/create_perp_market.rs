use anchor_lang::prelude::*;
use crate::constants::*;
use crate::error::ErrorCode;
use crate::events::PerpMarketCreated;
use crate::state::{Exchange, PerpMarket};

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CreatePerpMarketParams {
    pub market_index: u16,
    pub max_leverage: u64,
    pub min_position_size: u64,
    pub max_open_interest: u64,
}

#[derive(Accounts)]
#[instruction(params: CreatePerpMarketParams)]
pub struct CreatePerpMarket<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [EXCHANGE_SEED],
        bump = exchange.bump,
        constraint = exchange.admin == admin.key() @ ErrorCode::Unauthorized,
    )]
    pub exchange: Account<'info, Exchange>,

    #[account(
        init,
        payer = admin,
        space = PerpMarket::LEN,
        seeds = [PERP_MARKET_SEED, &params.market_index.to_le_bytes()],
        bump,
    )]
    pub perp_market: Account<'info, PerpMarket>,

    /// The base token mint (e.g., SOL)
    pub base_mint: Account<'info, anchor_spl::token::Mint>,

    /// The quote token mint (e.g., USDC)
    pub quote_mint: Account<'info, anchor_spl::token::Mint>,

    /// CHECK: Pyth oracle price feed account, validated by CPI at runtime
    pub oracle: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<CreatePerpMarket>, params: CreatePerpMarketParams) -> Result<()> {
    require!(
        params.max_leverage >= MIN_LEVERAGE && params.max_leverage <= MAX_LEVERAGE,
        ErrorCode::LeverageOutOfBounds
    );

    // Capture keys before mutable borrows
    let market_key = ctx.accounts.perp_market.key();
    let exchange_key = ctx.accounts.exchange.key();
    let base_mint_key = ctx.accounts.base_mint.key();
    let quote_mint_key = ctx.accounts.quote_mint.key();
    let oracle_key = ctx.accounts.oracle.key();
    let now = Clock::get()?.unix_timestamp;

    let market = &mut ctx.accounts.perp_market;
    market.exchange = exchange_key;
    market.bump = ctx.bumps.perp_market;
    market.base_mint = base_mint_key;
    market.quote_mint = quote_mint_key;
    market.market_index = params.market_index;
    market.oracle = oracle_key;
    market.max_leverage = params.max_leverage;
    market.min_position_size = params.min_position_size;
    market.long_open_interest = 0;
    market.short_open_interest = 0;
    market.max_open_interest = params.max_open_interest;
    market.funding_rate = 0;
    market.cumulative_funding_long = 0;
    market.cumulative_funding_short = 0;
    market.last_funding_update = now;
    market.paused = false;
    market._reserved = [0u8; 128];

    let exchange = &mut ctx.accounts.exchange;
    exchange.total_perp_markets = exchange
        .total_perp_markets
        .checked_add(1)
        .ok_or(ErrorCode::MathOverflow)?;

    emit!(PerpMarketCreated {
        market: market_key,
        base_mint: base_mint_key,
        quote_mint: quote_mint_key,
        timestamp: now,
    });

    Ok(())
}
