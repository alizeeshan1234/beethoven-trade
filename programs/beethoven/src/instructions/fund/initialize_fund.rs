use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};
use crate::constants::*;
use crate::error::ErrorCode;
use crate::events::FundInitialized;
use crate::state::{Exchange, Fund, FundStatus};

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct InitializeFundParams {
    pub performance_fee_bps: u64,
    pub management_fee_bps: u64,
    pub fee_recipient: Pubkey,
    pub meta_dao_program: Pubkey,
}

#[derive(Accounts)]
pub struct InitializeFund<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        seeds = [EXCHANGE_SEED],
        bump = exchange.bump,
        constraint = exchange.admin == admin.key() @ ErrorCode::Unauthorized,
    )]
    pub exchange: Account<'info, Exchange>,

    #[account(
        init,
        payer = admin,
        space = Fund::LEN,
        seeds = [FUND_SEED],
        bump,
    )]
    pub fund: Account<'info, Fund>,

    /// USDC mint (the fund's quote currency)
    pub quote_mint: Account<'info, Mint>,

    #[account(
        init,
        payer = admin,
        seeds = [SHARE_MINT_SEED],
        bump,
        mint::decimals = 6,
        mint::authority = fund,
    )]
    pub share_mint: Account<'info, Mint>,

    #[account(
        init,
        payer = admin,
        seeds = [FUND_VAULT_SEED],
        bump,
        token::mint = quote_mint,
        token::authority = fund,
    )]
    pub fund_vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn handler(ctx: Context<InitializeFund>, params: InitializeFundParams) -> Result<()> {
    require!(
        params.performance_fee_bps <= MAX_PERFORMANCE_FEE_BPS,
        ErrorCode::FeeExceedsMaximum
    );
    require!(
        params.management_fee_bps <= MAX_MANAGEMENT_FEE_BPS,
        ErrorCode::FeeExceedsMaximum
    );

    let clock = Clock::get()?;
    let fund = &mut ctx.accounts.fund;

    fund.admin = ctx.accounts.admin.key();
    fund.bump = ctx.bumps.fund;
    fund.quote_mint = ctx.accounts.quote_mint.key();
    fund.share_mint = ctx.accounts.share_mint.key();
    fund.fund_vault = ctx.accounts.fund_vault.key();
    fund.total_deposits = 0;
    fund.total_shares = 0;
    fund.nav_per_share = INITIAL_NAV_PER_SHARE;
    fund.total_nav = 0;
    fund.performance_fee_bps = params.performance_fee_bps;
    fund.management_fee_bps = params.management_fee_bps;
    fund.fee_recipient = params.fee_recipient;
    fund.total_proposals = 0;
    fund.active_proposals = 0;
    fund.total_holdings = 0;
    fund.meta_dao_program = params.meta_dao_program;
    fund.status = FundStatus::Active;
    fund.created_at = clock.unix_timestamp;
    fund.last_nav_update = clock.unix_timestamp;
    fund.last_fee_collection = clock.unix_timestamp;
    fund.high_water_mark = INITIAL_NAV_PER_SHARE;
    fund._reserved = [0u8; 128];

    emit!(FundInitialized {
        fund: ctx.accounts.fund.key(),
        admin: ctx.accounts.admin.key(),
        quote_mint: ctx.accounts.quote_mint.key(),
        share_mint: ctx.accounts.share_mint.key(),
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}
