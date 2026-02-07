use anchor_lang::prelude::*;
use crate::constants::*;
use crate::error::ErrorCode;
use crate::events::FeesCollected;
use crate::state::{Exchange, VaultState};

use anchor_spl::token::{TokenAccount, Token};

#[derive(Accounts)]
pub struct CollectFees<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        seeds = [EXCHANGE_SEED],
        bump = exchange.bump,
        constraint = exchange.admin == admin.key() @ ErrorCode::Unauthorized,
    )]
    pub exchange: Account<'info, Exchange>,

    #[account(
        mut,
        seeds = [VAULT_SEED, vault_state.mint.as_ref()],
        bump = vault_state.bump,
    )]
    pub vault_state: Account<'info, VaultState>,

    #[account(
        mut,
        constraint = vault_token_account.key() == vault_state.token_account @ ErrorCode::InvalidParameter,
    )]
    pub vault_token_account: Account<'info, TokenAccount>,

    /// The admin's token account to receive fees
    #[account(mut)]
    pub recipient_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

pub fn handler(ctx: Context<CollectFees>, amount: u64) -> Result<()> {
    require!(amount > 0, ErrorCode::InvalidAmount);

    let vault_state = &mut ctx.accounts.vault_state;
    require!(
        vault_state.collected_fees >= amount,
        ErrorCode::InsufficientVaultBalance
    );

    vault_state.collected_fees = vault_state
        .collected_fees
        .checked_sub(amount)
        .ok_or(ErrorCode::MathUnderflow)?;

    // Transfer fees from vault to recipient
    let mint_key = vault_state.mint;
    let seeds = &[
        VAULT_SEED,
        mint_key.as_ref(),
        &[vault_state.bump],
    ];
    let signer_seeds = &[&seeds[..]];

    anchor_spl::token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            anchor_spl::token::Transfer {
                from: ctx.accounts.vault_token_account.to_account_info(),
                to: ctx.accounts.recipient_token_account.to_account_info(),
                authority: vault_state.to_account_info(),
            },
            signer_seeds,
        ),
        amount,
    )?;

    let clock = Clock::get()?;
    emit!(FeesCollected {
        vault: ctx.accounts.vault_state.key(),
        amount,
        recipient: ctx.accounts.recipient_token_account.key(),
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}
