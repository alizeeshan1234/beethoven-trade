use anchor_lang::prelude::*;

// ── Protocol program IDs ─────────────────────────────────────────────
// Sourced from the beethoven SDK protocol crates (feature-gated).
// Beethoven detects which protocol to route to by checking
// remaining_accounts[0] against these known program IDs.
//
// Protocols are opt-in via Cargo.toml feature flags:
//   beethoven-sdk = { ..., features = ["manifest-swap", "kamino-deposit", ...] }
//
// See: https://blueshift.gg/research/composable-defi-with-beethoven

/// Swap protocol program IDs (from beethoven swap crates)
#[allow(unused_imports)]
pub mod swap_protocols {
    use anchor_lang::solana_program::pubkey::Pubkey;
    use anchor_lang::pubkey;

    // IDs sourced directly from beethoven's protocol crate constants.
    // When a new protocol is added to beethoven, add its feature flag
    // to Cargo.toml and its program ID here.

    /// Manifest DEX — `beethoven_sdk::manifest::MANIFEST_PROGRAM_ID`
    #[cfg(feature = "manifest-swap")]
    pub const MANIFEST: Pubkey = pubkey!("MNFSTqtC93rEfYHB6hF82sKdZpUDFWkViLByLd1k1Ms");

    /// Perena Numéraire — `beethoven_sdk::perena::PERENA_PROGRAM_ID`
    #[cfg(feature = "perena-swap")]
    pub const PERENA: Pubkey = pubkey!("NUMERUNsFCP3kuNmWZuXtm1AaQCPj9uw6Guv2Ekoi5P");

    /// Heaven Protocol — `beethoven_sdk::heaven::HEAVEN_PROGRAM_ID`
    #[cfg(feature = "heaven-swap")]
    pub const HEAVEN: Pubkey = pubkey!("HEAVENoP2qxoeuF8Dj2oT1GHEnu49U5mJYkdeC8BAX2o");

    /// Aldrin DEX — `beethoven_sdk::aldrin::ALDRIN_PROGRAM_ID`
    #[cfg(feature = "aldrin-swap")]
    pub const ALDRIN: Pubkey = pubkey!("AMM55ShdkoGRB5jVYPjWziwk8m5MpwyDgsMWHaMSQWH6");

    /// Gamma Protocol — `beethoven_sdk::gamma::GAMMA_PROGRAM_ID`
    #[cfg(feature = "gamma-swap")]
    pub const GAMMA: Pubkey = pubkey!("GAMMA7meSFWaBXF25oSUgmGRwaW6sCMFLmBNiMSdbHVT");

    /// SolFi — `beethoven_sdk::solfi::SOLFI_PROGRAM_ID`
    #[cfg(feature = "solfi-swap")]
    pub const SOLFI: Pubkey = pubkey!("SoLFiHG9TfgtdUXUjWAxi3LtvYuFyDLVhBWxdMZxyCe");

    /// Futarchy — `beethoven_sdk::futarchy::FUTARCHY_PROGRAM_ID`
    #[cfg(feature = "futarchy-swap")]
    pub const FUTARCHY: Pubkey = pubkey!("FUTARELBfJfQ8RDGhg1wdhddq1odMAJUePHFuBYfUxKq");
}

/// Deposit protocol program IDs (from beethoven deposit crates)
/// Note: Kamino and Jupiter IDs are placeholders in current beethoven source ([0; 32]).
/// Replace with actual program IDs when beethoven updates them.
#[allow(unused_imports)]
pub mod deposit_protocols {
    use anchor_lang::solana_program::pubkey::Pubkey;
    use anchor_lang::pubkey;

    /// Kamino Finance klend — devnet program ID
    #[cfg(feature = "kamino-deposit")]
    pub const KAMINO: Pubkey = pubkey!("KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD");

    /// Jupiter Earn — `beethoven_sdk::jupiter::JUPITER_EARN_PROGRAM_ID`
    /// Devnet program ID for Jupiter Earn (lending/yield)
    #[cfg(feature = "jupiter-deposit")]
    pub const JUPITER: Pubkey = pubkey!("7tjE28izRUjzmxC1QNXnNwcc4N82CNYCexf3k8mw67s3");
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SwapProtocol {
    #[cfg(feature = "manifest-swap")]
    Manifest,
    #[cfg(feature = "perena-swap")]
    Perena,
    #[cfg(feature = "heaven-swap")]
    Heaven,
    #[cfg(feature = "aldrin-swap")]
    Aldrin,
    #[cfg(feature = "gamma-swap")]
    Gamma,
    #[cfg(feature = "solfi-swap")]
    SolFi,
    #[cfg(feature = "futarchy-swap")]
    Futarchy,
    Unknown,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DepositProtocol {
    #[cfg(feature = "kamino-deposit")]
    Kamino,
    #[cfg(feature = "jupiter-deposit")]
    Jupiter,
    Unknown,
}

/// Detect which swap protocol from program ID (remaining_accounts[0]).
/// Mirrors beethoven's `try_from_swap_context` detection pattern.
pub fn detect_swap_protocol(program_id: &Pubkey) -> SwapProtocol {
    #[cfg(feature = "manifest-swap")]
    if *program_id == swap_protocols::MANIFEST {
        return SwapProtocol::Manifest;
    }
    #[cfg(feature = "perena-swap")]
    if *program_id == swap_protocols::PERENA {
        return SwapProtocol::Perena;
    }
    #[cfg(feature = "heaven-swap")]
    if *program_id == swap_protocols::HEAVEN {
        return SwapProtocol::Heaven;
    }
    #[cfg(feature = "aldrin-swap")]
    if *program_id == swap_protocols::ALDRIN {
        return SwapProtocol::Aldrin;
    }
    #[cfg(feature = "gamma-swap")]
    if *program_id == swap_protocols::GAMMA {
        return SwapProtocol::Gamma;
    }
    #[cfg(feature = "solfi-swap")]
    if *program_id == swap_protocols::SOLFI {
        return SwapProtocol::SolFi;
    }
    #[cfg(feature = "futarchy-swap")]
    if *program_id == swap_protocols::FUTARCHY {
        return SwapProtocol::Futarchy;
    }
    let _ = program_id;
    SwapProtocol::Unknown
}

/// Detect which deposit protocol from program ID
pub fn detect_deposit_protocol(program_id: &Pubkey) -> DepositProtocol {
    #[cfg(feature = "kamino-deposit")]
    if *program_id == deposit_protocols::KAMINO {
        return DepositProtocol::Kamino;
    }
    #[cfg(feature = "jupiter-deposit")]
    if *program_id == deposit_protocols::JUPITER {
        return DepositProtocol::Jupiter;
    }
    let _ = program_id;
    DepositProtocol::Unknown
}
