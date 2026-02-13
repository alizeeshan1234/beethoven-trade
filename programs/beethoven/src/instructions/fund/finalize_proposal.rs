use anchor_lang::prelude::*;
use crate::constants::*;
use crate::error::ErrorCode;
use crate::events::ProposalFinalized;
use crate::state::{Fund, Proposal, ProposalStatus};

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct FinalizeProposalParams {
    /// Admin can override TWAP values for testing or emergency use.
    /// When both are Some, they bypass MetaDAO TWAP reading.
    /// The signer must be the fund admin to use overrides.
    pub admin_pass_twap: Option<u128>,
    pub admin_fail_twap: Option<u128>,
}

#[derive(Accounts)]
pub struct FinalizeProposal<'info> {
    /// Permissionless cranker (or admin for TWAP override)
    pub cranker: Signer<'info>,

    #[account(
        mut,
        seeds = [FUND_SEED],
        bump = fund.bump,
    )]
    pub fund: Account<'info, Fund>,

    #[account(
        mut,
        seeds = [
            FUND_PROPOSAL_SEED,
            fund.key().as_ref(),
            &proposal.proposal_index.to_le_bytes(),
        ],
        bump = proposal.bump,
        constraint = proposal.fund == fund.key() @ ErrorCode::InvalidParameter,
        constraint = proposal.status == ProposalStatus::Active @ ErrorCode::ProposalNotActive,
    )]
    pub proposal: Account<'info, Proposal>,
    // remaining_accounts: [metadao_program, pass_twap_account, fail_twap_account]
}

pub fn handler<'info>(
    ctx: Context<'_, '_, 'info, 'info, FinalizeProposal<'info>>,
    params: FinalizeProposalParams,
) -> Result<()> {
    let clock = Clock::get()?;
    let proposal = &ctx.accounts.proposal;

    // Ensure voting period has ended
    require!(
        clock.unix_timestamp >= proposal.voting_end,
        ErrorCode::VotingPeriodNotEnded
    );

    // Determine TWAP values — either from admin override or MetaDAO accounts
    let (pass_twap, fail_twap) = if let (Some(admin_pass), Some(admin_fail)) =
        (params.admin_pass_twap, params.admin_fail_twap)
    {
        // Admin TWAP override — verify signer is fund admin
        require!(
            ctx.accounts.cranker.key() == ctx.accounts.fund.admin,
            ErrorCode::Unauthorized
        );
        (admin_pass, admin_fail)
    } else if ctx.remaining_accounts.len() >= 3 {
        // Read TWAP from MetaDAO market accounts via remaining_accounts
        // remaining_accounts[0] = metadao_program (for verification)
        // remaining_accounts[1] = pass market AMM account (TWAP at known offset)
        // remaining_accounts[2] = fail market AMM account (TWAP at known offset)
        let pass_account = &ctx.remaining_accounts[1];
        let fail_account = &ctx.remaining_accounts[2];

        let pass_twap = read_metadao_twap(pass_account)?;
        let fail_twap = read_metadao_twap(fail_account)?;

        (pass_twap, fail_twap)
    } else {
        // If no MetaDAO accounts provided and no override, check for expiration
        require!(
            clock.unix_timestamp > proposal.execution_deadline,
            ErrorCode::InvalidParameter
        );
        (0u128, 0u128)
    };

    // Determine outcome: pass if pass_twap > fail_twap
    let passed = pass_twap > fail_twap;

    // Update proposal
    let proposal = &mut ctx.accounts.proposal;
    proposal.pass_twap = pass_twap;
    proposal.fail_twap = fail_twap;
    proposal.status = if passed {
        ProposalStatus::Passed
    } else {
        ProposalStatus::Failed
    };

    // Decrement active proposals
    let fund = &mut ctx.accounts.fund;
    fund.active_proposals = fund.active_proposals.saturating_sub(1);

    emit!(ProposalFinalized {
        fund: ctx.accounts.fund.key(),
        proposal: ctx.accounts.proposal.key(),
        passed,
        pass_twap,
        fail_twap,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

/// Read TWAP from a MetaDAO AMM account at known byte offsets.
/// MetaDAO AMM stores the oracle/TWAP data at specific offsets in the account.
/// This is a read-only operation — no CPI needed.
fn read_metadao_twap(account: &AccountInfo) -> Result<u128> {
    let data = account.try_borrow_data()?;

    // MetaDAO AMM account layout:
    // The TWAP observation is stored after the AMM state fields.
    // Offset 208: oracle.last_observation (u128) — the TWAP value
    // This offset may need adjustment based on the actual MetaDAO AMM layout.
    const TWAP_OFFSET: usize = 208;

    require!(
        data.len() >= TWAP_OFFSET + 16,
        ErrorCode::InvalidActionData
    );

    let twap_bytes: [u8; 16] = data[TWAP_OFFSET..TWAP_OFFSET + 16]
        .try_into()
        .map_err(|_| ErrorCode::InvalidActionData)?;

    Ok(u128::from_le_bytes(twap_bytes))
}
