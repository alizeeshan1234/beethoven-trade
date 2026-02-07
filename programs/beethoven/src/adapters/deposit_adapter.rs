use anchor_lang::prelude::*;
use crate::adapters::protocol_ids::{detect_deposit_protocol, DepositProtocol};
#[allow(unused_imports)]
use crate::adapters::account_bridge::invoke_protocol_cpi;
use crate::error::ErrorCode;

/// Execute a deposit via Beethoven's composable routing pattern.
///
/// remaining_accounts layout:
///   [0] = Protocol program ID (executable)
///   [1..] = Protocol-specific accounts
///
/// See: https://blueshift.gg/research/composable-defi-with-beethoven
#[allow(unused_variables)]
pub fn execute_deposit<'info>(
    remaining_accounts: &[AccountInfo<'info>],
    amount: u64,
) -> Result<()> {
    require!(
        !remaining_accounts.is_empty(),
        ErrorCode::UnsupportedProtocol
    );

    let protocol_program = &remaining_accounts[0];
    let protocol = detect_deposit_protocol(protocol_program.key);

    require!(
        protocol != DepositProtocol::Unknown,
        ErrorCode::UnsupportedProtocol
    );

    let accounts = &remaining_accounts[1..];

    match protocol {
        #[cfg(feature = "kamino-deposit")]
        DepositProtocol::Kamino => {
            // Kamino klend depositReserveLiquidity:
            //   discriminator(8) + liquidityAmount(8) = 16 bytes
            //   Accounts (9): owner, reserve, lendingMarket, lendingMarketAuthority,
            //     reserveLiquiditySupply, reserveCollateralMint,
            //     userSourceLiquidity, userDestinationCollateral, tokenProgram
            require!(accounts.len() >= 9, ErrorCode::InvalidParameter);
            let mut data = Vec::with_capacity(16);
            data.extend_from_slice(&[169, 201, 30, 126, 6, 205, 102, 68]); // depositReserveLiquidity
            data.extend_from_slice(&amount.to_le_bytes());
            invoke_protocol_cpi(protocol_program, accounts, data)
        }
        #[cfg(feature = "jupiter-deposit")]
        DepositProtocol::Jupiter => {
            require!(accounts.len() >= 4, ErrorCode::InvalidParameter);
            let mut data = Vec::with_capacity(16);
            data.extend_from_slice(&[0xd3, 0xb4, 0x06, 0x5e, 0xc4, 0xa3, 0x71, 0x5c]);
            data.extend_from_slice(&amount.to_le_bytes());
            invoke_protocol_cpi(protocol_program, accounts, data)
        }
        DepositProtocol::Unknown => Err(ErrorCode::UnsupportedProtocol.into()),
    }
}

/// Execute withdrawal from a yield protocol via Beethoven routing.
#[allow(unused_variables)]
pub fn execute_withdraw<'info>(
    remaining_accounts: &[AccountInfo<'info>],
    amount: u64,
) -> Result<()> {
    require!(
        !remaining_accounts.is_empty(),
        ErrorCode::UnsupportedProtocol
    );

    let protocol_program = &remaining_accounts[0];
    let protocol = detect_deposit_protocol(protocol_program.key);

    require!(
        protocol != DepositProtocol::Unknown,
        ErrorCode::UnsupportedProtocol
    );

    let accounts = &remaining_accounts[1..];

    match protocol {
        #[cfg(feature = "kamino-deposit")]
        DepositProtocol::Kamino => {
            // Kamino klend redeemReserveCollateral:
            //   discriminator(8) + collateralAmount(8) = 16 bytes
            //   Accounts (9): owner, lendingMarket, reserve, lendingMarketAuthority,
            //     reserveCollateralMint, reserveLiquiditySupply,
            //     userSourceCollateral, userDestinationLiquidity, tokenProgram
            require!(accounts.len() >= 9, ErrorCode::InvalidParameter);
            let mut data = Vec::with_capacity(16);
            data.extend_from_slice(&[234, 117, 181, 125, 185, 142, 220, 29]); // redeemReserveCollateral
            data.extend_from_slice(&amount.to_le_bytes());
            invoke_protocol_cpi(protocol_program, accounts, data)
        }
        #[cfg(feature = "jupiter-deposit")]
        DepositProtocol::Jupiter => {
            require!(accounts.len() >= 4, ErrorCode::InvalidParameter);
            let mut data = Vec::with_capacity(16);
            data.extend_from_slice(&[0xa1, 0xc2, 0xd3, 0xe4, 0xf5, 0x06, 0x17, 0x28]);
            data.extend_from_slice(&amount.to_le_bytes());
            invoke_protocol_cpi(protocol_program, accounts, data)
        }
        DepositProtocol::Unknown => Err(ErrorCode::UnsupportedProtocol.into()),
    }
}

