use anchor_lang::prelude::*;
use crate::constants::*;
use crate::error::ErrorCode;
use crate::events::LiquidityRemoved;
use crate::adapters::deposit_adapter;
use crate::state::{Exchange, UserAccount};

#[derive(Accounts)]
pub struct RemoveLiquidity<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        seeds = [EXCHANGE_SEED],
        bump = exchange.bump,
    )]
    pub exchange: Account<'info, Exchange>,

    #[account(
        mut,
        seeds = [USER_ACCOUNT_SEED, user.key().as_ref()],
        bump = user_account.bump,
        constraint = user_account.owner == user.key() @ ErrorCode::Unauthorized,
    )]
    pub user_account: Account<'info, UserAccount>,

    /// User's token account for receiving the withdrawal
    #[account(mut)]
    pub user_token_account: Account<'info, anchor_spl::token::TokenAccount>,

    pub token_program: Program<'info, anchor_spl::token::Token>,
}

pub fn handler<'info>(
    ctx: Context<'_, '_, 'info, 'info, RemoveLiquidity<'info>>,
    amount: u64,
) -> Result<()> {
    require!(amount > 0, ErrorCode::InvalidAmount);

    let clock = Clock::get()?;

    // Execute withdrawal via protocol adapter
    deposit_adapter::execute_withdraw(ctx.remaining_accounts, amount)?;

    // Update user stats
    let user = &mut ctx.accounts.user_account;
    user.last_activity = clock.unix_timestamp;

    // Get protocol from remaining accounts for event
    let protocol = if !ctx.remaining_accounts.is_empty() {
        *ctx.remaining_accounts[0].key
    } else {
        Pubkey::default()
    };

    emit!(LiquidityRemoved {
        user: ctx.accounts.user.key(),
        mint: ctx.accounts.user_token_account.mint,
        amount,
        protocol,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}
