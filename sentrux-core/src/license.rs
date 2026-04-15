//! License tier system.
//!
//! The `Tier` enum is the universal currency for feature gating.
//! The free open-source binary always returns `Tier::Free`.
//! Pro tier validation lives in a separate private repository
//! and plugs in via the `pro` Cargo feature at build time.

use serde::{Deserialize, Serialize};

/// License tier determining feature access.
///
/// Ordered by privilege: Free < Pro < Team.
/// Pro/Team tiers are activated by an optional integration crate
/// which provides Ed25519 license key validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Free = 0,
    Pro = 1,
    Team = 2,
}

impl Tier {
    /// Check if this tier grants access to features requiring `required` tier.
    #[inline]
    pub fn can_access(self, required: Tier) -> bool {
        self >= required
    }

    #[inline]
    pub fn is_pro(self) -> bool {
        self >= Tier::Pro
    }

    #[inline]
    pub fn is_team(self) -> bool {
        self >= Tier::Team
    }

    /// Detail list limit for this tier (used by health, test_gaps, etc.)
    pub fn detail_limit(self) -> usize {
        match self {
            Tier::Free => 0,
            Tier::Pro | Tier::Team => usize::MAX,
        }
    }
}

impl std::fmt::Display for Tier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Tier::Free => write!(f, "Free"),
            Tier::Pro => write!(f, "Pro"),
            Tier::Team => write!(f, "Team"),
        }
    }
}

/// Load the current license tier.
///
/// In the open-source build, always returns `Tier::Free`.
/// The Pro build overrides this by calling `set_tier()` at startup.
static TIER: std::sync::OnceLock<Tier> = std::sync::OnceLock::new();

/// Set the tier (called by sentrux-bin with pro feature at startup).
pub fn set_tier(tier: Tier) {
    let _ = TIER.set(tier);
}

/// Get the current tier.
pub fn current_tier() -> Tier {
    *TIER.get().unwrap_or(&Tier::Free)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_ordering() {
        assert!(Tier::Free < Tier::Pro);
        assert!(Tier::Pro < Tier::Team);
    }

    #[test]
    fn can_access_logic() {
        assert!(Tier::Pro.can_access(Tier::Free));
        assert!(Tier::Pro.can_access(Tier::Pro));
        assert!(!Tier::Pro.can_access(Tier::Team));
        assert!(Tier::Team.can_access(Tier::Team));
        assert!(!Tier::Free.can_access(Tier::Pro));
    }

    #[test]
    fn detail_limits() {
        assert_eq!(Tier::Free.detail_limit(), 0);
        assert_eq!(Tier::Pro.detail_limit(), usize::MAX);
        assert_eq!(Tier::Team.detail_limit(), usize::MAX);
    }

    #[test]
    fn display() {
        assert_eq!(Tier::Free.to_string(), "Free");
        assert_eq!(Tier::Pro.to_string(), "Pro");
        assert_eq!(Tier::Team.to_string(), "Team");
    }

    #[test]
    fn free_tier_default() {
        assert_eq!(current_tier(), Tier::Free);
    }
}
