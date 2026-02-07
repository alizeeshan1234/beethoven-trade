use anchor_lang::prelude::*;
use crate::constants::*;
use crate::error::ErrorCode;
use crate::events::PerpPositionOpened;
use crate::math::liquidation::compute_liquidation_price;
use crate::math::oracle::get_price;
use crate::state::{Exchange, PerpMarket, PerpPosition, UserAccount};
use crate::state::perp_position::PositionSide;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct OpenPositionParams {
    pub is_long: bool,
    pub size: u64,      // Position size in base units
    pub collateral: u64, // Collateral amount in quote units
}

#[derive(Accounts)]
pub struct OpenPosition<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        seeds = [EXCHANGE_SEED],
        bump = exchange.bump,
        constraint = !exchange.perp_paused @ ErrorCode::ExchangePaused,
    )]
    pub exchange: Box<Account<'info, Exchange>>,

    #[account(
        mut,
        seeds = [USER_ACCOUNT_SEED, owner.key().as_ref()],
        bump = user_account.bump,
        constraint = user_account.owner == owner.key() @ ErrorCode::Unauthorized,
    )]
    pub user_account: Box<Account<'info, UserAccount>>,

    #[account(
        mut,
        seeds = [PERP_MARKET_SEED, &perp_market.market_index.to_le_bytes()],
        bump = perp_market.bump,
        constraint = !perp_market.paused @ ErrorCode::ExchangePaused,
    )]
    pub perp_market: Box<Account<'info, PerpMarket>>,

    #[account(
        init,
        payer = owner,
        space = PerpPosition::LEN,
        seeds = [
            PERP_POSITION_SEED,
            owner.key().as_ref(),
            perp_market.key().as_ref(),
            &user_account.open_perp_positions.to_le_bytes(),
        ],
        bump,
    )]
    pub perp_position: Account<'info, PerpPosition>,

    /// CHECK: Pyth oracle price feed
    #[account(
        constraint = oracle.key() == perp_market.oracle @ ErrorCode::OracleAccountMismatch,
    )]
    pub oracle: UncheckedAccount<'info>,

    /// User's quote token account (collateral source)
    #[account(
        mut,
        constraint = user_token_account.owner == owner.key() @ ErrorCode::Unauthorized,
    )]
    pub user_token_account: Account<'info, anchor_spl::token::TokenAccount>,

    /// Vault token account (collateral destination)
    #[account(mut)]
    pub vault_token_account: Account<'info, anchor_spl::token::TokenAccount>,

    #[account(
        seeds = [VAULT_SEED, perp_market.quote_mint.as_ref()],
        bump,
    )]
    pub vault_state: Account<'info, crate::state::VaultState>,

    pub token_program: Program<'info, anchor_spl::token::Token>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<OpenPosition>, params: OpenPositionParams) -> Result<()> {
    require!(params.size > 0, ErrorCode::PositionTooSmall);
    require!(params.collateral > 0, ErrorCode::InsufficientCollateral);

    let market = &ctx.accounts.perp_market;
    let exchange = &ctx.accounts.exchange;
    let clock = Clock::get()?;

    // Check position limits
    require!(
        ctx.accounts.user_account.open_perp_positions < MAX_PERP_POSITIONS,
        ErrorCode::MaxPerpPositionsReached
    );

    // Get oracle price
    let oracle_price = get_price(&ctx.accounts.oracle.to_account_info(), &clock)?;

    // Calculate leverage: leverage = (size * price / PRICE_PRECISION) / collateral
    let notional = (params.size as u128)
        .checked_mul(oracle_price.price as u128)
        .ok_or(ErrorCode::MathOverflow)?
        .checked_div(PRICE_PRECISION as u128)
        .ok_or(ErrorCode::DivisionByZero)?;

    let leverage = notional
        .checked_div(params.collateral as u128)
        .ok_or(ErrorCode::DivisionByZero)?;

    let max_lev = market.max_leverage.min(exchange.max_leverage);
    require!(leverage <= max_lev as u128, ErrorCode::ExcessiveLeverage);
    require!(leverage >= MIN_LEVERAGE as u128, ErrorCode::ExcessiveLeverage);

    // Check min position size
    require!(
        params.size >= market.min_position_size,
        ErrorCode::PositionTooSmall
    );

    // Check OI limits
    let side = if params.is_long {
        PositionSide::Long
    } else {
        PositionSide::Short
    };

    // Transfer collateral from user to vault
    anchor_spl::token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            anchor_spl::token::Transfer {
                from: ctx.accounts.user_token_account.to_account_info(),
                to: ctx.accounts.vault_token_account.to_account_info(),
                authority: ctx.accounts.owner.to_account_info(),
            },
        ),
        params.collateral,
    )?;

    // Compute liquidation price
    let liq_price = compute_liquidation_price(
        &side,
        oracle_price.price,
        params.collateral,
        params.size,
    )?;

    // Update market OI
    let market = &mut ctx.accounts.perp_market;
    match side {
        PositionSide::Long => {
            let new_oi = market
                .long_open_interest
                .checked_add(params.size)
                .ok_or(ErrorCode::MathOverflow)?;
            require!(
                new_oi <= market.max_open_interest,
                ErrorCode::OpenInterestLimitExceeded
            );
            market.long_open_interest = new_oi;
        }
        PositionSide::Short => {
            let new_oi = market
                .short_open_interest
                .checked_add(params.size)
                .ok_or(ErrorCode::MathOverflow)?;
            require!(
                new_oi <= market.max_open_interest,
                ErrorCode::OpenInterestLimitExceeded
            );
            market.short_open_interest = new_oi;
        }
    }

    // Funding snapshot
    let funding_snapshot = match side {
        PositionSide::Long => market.cumulative_funding_long,
        PositionSide::Short => market.cumulative_funding_short,
    };

    // Initialize position
    let position = &mut ctx.accounts.perp_position;
    position.owner = ctx.accounts.owner.key();
    position.market = ctx.accounts.perp_market.key();
    position.bump = ctx.bumps.perp_position;
    position.side = side;
    position.size = params.size;
    position.collateral = params.collateral;
    position.entry_price = oracle_price.price;
    position.leverage = leverage as u64;
    position.cumulative_funding_snapshot = funding_snapshot;
    position.liquidation_price = liq_price;
    position.realized_pnl = 0;
    position.unrealized_pnl = 0;
    position.opened_at = clock.unix_timestamp;
    position.last_updated = clock.unix_timestamp;
    position._reserved = [0u8; 64];

    // Update user account
    let user = &mut ctx.accounts.user_account;
    user.open_perp_positions = user
        .open_perp_positions
        .checked_add(1)
        .ok_or(ErrorCode::MaxPerpPositionsReached)?;
    user.total_trades = user
        .total_trades
        .checked_add(1)
        .ok_or(ErrorCode::MathOverflow)?;
    user.total_volume = user
        .total_volume
        .checked_add(notional as u64)
        .ok_or(ErrorCode::MathOverflow)?;
    user.last_activity = clock.unix_timestamp;

    emit!(PerpPositionOpened {
        user: ctx.accounts.owner.key(),
        market: ctx.accounts.perp_market.key(),
        is_long: params.is_long,
        size: params.size,
        collateral: params.collateral,
        entry_price: oracle_price.price,
        leverage: leverage as u64,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}
