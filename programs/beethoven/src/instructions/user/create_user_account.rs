use anchor_lang::prelude::*;
use crate::constants::*;
use crate::error::ErrorCode;
use crate::state::{Exchange, UserAccount};

#[derive(Accounts)]
pub struct CreateUserAccount<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        seeds = [EXCHANGE_SEED],
        bump = exchange.bump,
    )]
    pub exchange: Account<'info, Exchange>,

    #[account(
        init,
        payer = owner,
        space = UserAccount::LEN,
        seeds = [USER_ACCOUNT_SEED, owner.key().as_ref()],
        bump,
    )]
    pub user_account: Account<'info, UserAccount>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<CreateUserAccount>, referrer: Option<Pubkey>) -> Result<()> {
    let user = &mut ctx.accounts.user_account;
    let clock = Clock::get()?;

    user.owner = ctx.accounts.owner.key();
    user.bump = ctx.bumps.user_account;
    user.open_perp_positions = 0;
    user.open_lending_positions = 0;
    user.total_trades = 0;
    user.total_pnl = 0;
    user.total_volume = 0;
    user.total_fees_paid = 0;
    user.referrer = referrer.unwrap_or_default();
    user.created_at = clock.unix_timestamp;
    user.last_activity = clock.unix_timestamp;
    user._reserved = [0u8; 64];

    let exchange = &mut ctx.accounts.exchange;
    exchange.total_users = exchange
        .total_users
        .checked_add(1)
        .ok_or(ErrorCode::MathOverflow)?;

    Ok(())
}
