//! How the game plays, a port feature: as the original (the classic mode)
//! or with the port's enhancements, conveniences the original never had
//! that the player turns on one by one. The launcher chooses the mode
//! before the game starts; the classic mode ignores every enhancement.
//!
//! Source of knowledge: this project's own design.

/// How the game plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlayMode {
    /// As the original: no enhancement applies.
    #[default]
    Classic,
    /// With the enhancements chosen.
    Enhanced(Enhancements),
}

/// The conveniences the enhanced mode can turn on or off, each as the
/// original plays by default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Enhancements {
    /// Whether the battles show their attack scenes: without them an
    /// enemy's attack goes straight to its outcome and the party's scene
    /// stops once the player has aimed.
    pub battle_animations: bool,
}

impl Default for Enhancements {
    fn default() -> Self {
        Self {
            battle_animations: true,
        }
    }
}

impl PlayMode {
    /// The enhancements that apply: the original's behavior in the classic
    /// mode.
    #[must_use]
    pub fn enhancements(self) -> Enhancements {
        match self {
            Self::Classic => Enhancements::default(),
            Self::Enhanced(enhancements) => enhancements,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_classic_mode_ignores_the_enhancements_chosen() {
        let chosen = Enhancements {
            battle_animations: false,
        };
        assert!(PlayMode::Classic.enhancements().battle_animations);
        assert!(!PlayMode::Enhanced(chosen).enhancements().battle_animations);
    }

    #[test]
    fn the_enhancements_start_as_the_original_plays() {
        assert_eq!(PlayMode::default(), PlayMode::Classic);
        assert!(Enhancements::default().battle_animations);
    }
}
