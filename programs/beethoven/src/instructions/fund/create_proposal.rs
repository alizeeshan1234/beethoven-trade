use anchor_lang::prelude::*;
use anchor_spl::token::TokenAccount;
use crate::constants::*;
use crate::error::ErrorCode;
use crate::events::ProposalCreated;
use crate::state::{Fund, FundStatus, Proposal, ProposalStatus, ActionType};

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CreateProposalParams {
    pub action_type: ActionType,
    pub action_data: [u8; 256],
    pub pass_market: Pubkey,
    pub fail_market: Pubkey,
}

#[derive(Accounts)]
pub struct CreateProposal<'info> {
    #[account(mut)]
    pub proposer: Signer<'info>,

    #[account(
        mut,
        seeds = [FUND_SEED],
        bump = fund.bump,
        constraint = fund.status == FundStatus::Active @ ErrorCode::FundPaused,
        constraint = fund.active_proposals < MAX_ACTIVE_PROPOSALS @ ErrorCode::MaxActiveProposals,
    )]
    pub fund: Account<'info, Fund>,

    /// Proposer's share token account — must hold >= MIN_PROPOSAL_SHARES
    #[account(
        constraint = proposer_share_account.mint == fund.share_mint @ ErrorCode::InvalidParameter,
        constraint = proposer_share_account.owner == proposer.key() @ ErrorCode::Unauthorized,
        constraint = proposer_share_account.amount >= MIN_PROPOSAL_SHARES @ ErrorCode::InsufficientShares,
    )]
    pub proposer_share_account: Account<'info, TokenAccount>,

    #[account(
        init,
        payer = proposer,
        space = Proposal::LEN,
        seeds = [
            FUND_PROPOSAL_SEED,
            fund.key().as_ref(),
            &fund.total_proposals.to_le_bytes(),
        ],
        bump,
    )]
    pub proposal: Account<'info, Proposal>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<CreateProposal>, params: CreateProposalParams) -> Result<()> {
    let clock = Clock::get()?;
    let fund = &ctx.accounts.fund;
    let proposal_index = fund.total_proposals;

    let voting_start = clock.unix_timestamp;
    let voting_end = voting_start
        .checked_add(PROPOSAL_VOTING_PERIOD)
        .ok_or(ErrorCode::MathOverflow)?;
    let execution_deadline = voting_end
        .checked_add(PROPOSAL_EXECUTION_DEADLINE)
        .ok_or(ErrorCode::MathOverflow)?;

    // Initialize proposal
    let proposal = &mut ctx.accounts.proposal;
    proposal.fund = ctx.accounts.fund.key();
    proposal.proposer = ctx.accounts.proposer.key();
    proposal.bump = ctx.bumps.proposal;
    proposal.proposal_index = proposal_index;
    proposal.action_type = params.action_type;
    proposal.action_data = params.action_data;
    proposal.pass_market = params.pass_market;
    proposal.fail_market = params.fail_market;
    proposal.pass_twap = 0;
    proposal.fail_twap = 0;
    proposal.status = ProposalStatus::Active;
    proposal.voting_start = voting_start;
    proposal.voting_end = voting_end;
    proposal.execution_deadline = execution_deadline;
    proposal.executed_at = 0;
    proposal._reserved = [0u8; 128];

    // Update fund counters
    let fund = &mut ctx.accounts.fund;
    fund.total_proposals = fund
        .total_proposals
        .checked_add(1)
        .ok_or(ErrorCode::MathOverflow)?;
    fund.active_proposals = fund
        .active_proposals
        .checked_add(1)
        .ok_or(ErrorCode::MaxActiveProposals)?;

    emit!(ProposalCreated {
        fund: ctx.accounts.fund.key(),
        proposal: ctx.accounts.proposal.key(),
        proposer: ctx.accounts.proposer.key(),
        proposal_index,
        action_type: params.action_type as u8,
        voting_end,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}
