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
/// original plays by default but the autosave and the weapons' reach,
/// which are on: they change nothing of the play. Each is a
/// switch of its own, on or off whatever the others are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct Enhancements {
    /// Whether the battles show their attack scenes: without them an
    /// enemy's attack goes straight to its outcome and the party's scene
    /// stops once the player has aimed.
    pub battle_animations: bool,
    /// Whether a battle shows the damage each unit takes as a number under
    /// it for a moment, besides the message.
    pub damage_numbers: bool,
    /// Whether the text boxes go on by themselves once their text has
    /// been on screen long enough to read, as a key would; SELECT turns it
    /// on or off while a text box shows.
    pub auto_text: bool,
    /// Whether the game is saved on each change of map, once the player
    /// walks freely on the new one, into a slot of its own that
    /// continuing lists first and saving never offers.
    pub autosave: bool,
    /// Whether the armaments shops and the pause menu's weapons draw what
    /// the weapon under the cursor reaches on the battle's grid, from the
    /// front row or the back, SELECT turning from one to the other.
    pub weapon_reach: bool,
    /// How many frames the fast forward plays for each one shown while its
    /// button is held, from [`FAST_FORWARD_SPEEDS`]; the launcher plays
    /// them, the game only keeps the choice.
    pub fast_forward: u8,
}

/// The speeds the fast forward can take, in frames per frame shown.
pub const FAST_FORWARD_SPEEDS: std::ops::RangeInclusive<u8> = 2..=4;

impl Default for Enhancements {
    fn default() -> Self {
        Self {
            battle_animations: true,
            damage_numbers: false,
            auto_text: false,
            autosave: true,
            weapon_reach: true,
            fast_forward: *FAST_FORWARD_SPEEDS.start(),
        }
    }
}

/// One of the enhanced mode's conveniences, each on or off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Enhancement {
    /// See [`Enhancements::battle_animations`].
    BattleAnimations,
    /// See [`Enhancements::damage_numbers`].
    DamageNumbers,
    /// See [`Enhancements::auto_text`].
    AutoText,
    /// See [`Enhancements::autosave`].
    Autosave,
    /// See [`Enhancements::weapon_reach`].
    WeaponReach,
}

impl Enhancement {
    /// Every enhancement, in the order the lists show them.
    pub const ALL: [Self; 5] = [
        Self::BattleAnimations,
        Self::DamageNumbers,
        Self::AutoText,
        Self::Autosave,
        Self::WeaponReach,
    ];
}

impl Enhancements {
    /// Whether `enhancement` is on.
    #[must_use]
    pub fn is_on(self, enhancement: Enhancement) -> bool {
        match enhancement {
            Enhancement::BattleAnimations => self.battle_animations,
            Enhancement::DamageNumbers => self.damage_numbers,
            Enhancement::AutoText => self.auto_text,
            Enhancement::Autosave => self.autosave,
            Enhancement::WeaponReach => self.weapon_reach,
        }
    }

    /// Turns `enhancement` on or off, the other way than it is.
    pub fn toggle(&mut self, enhancement: Enhancement) {
        let flag = match enhancement {
            Enhancement::BattleAnimations => &mut self.battle_animations,
            Enhancement::DamageNumbers => &mut self.damage_numbers,
            Enhancement::AutoText => &mut self.auto_text,
            Enhancement::Autosave => &mut self.autosave,
            Enhancement::WeaponReach => &mut self.weapon_reach,
        };
        *flag = !*flag;
    }
}

impl PlayMode {
    /// The enhancements that apply: the original's behavior in the classic
    /// mode, which neither autosaves nor draws the weapons' reach.
    #[must_use]
    pub fn enhancements(self) -> Enhancements {
        match self {
            Self::Classic => Enhancements {
                autosave: false,
                weapon_reach: false,
                ..Enhancements::default()
            },
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
            damage_numbers: true,
            auto_text: true,
            autosave: true,
            weapon_reach: true,
            fast_forward: 3,
        };
        assert!(PlayMode::Classic.enhancements().battle_animations);
        assert!(!PlayMode::Classic.enhancements().damage_numbers);
        assert!(!PlayMode::Classic.enhancements().autosave);
        assert!(!PlayMode::Classic.enhancements().weapon_reach);
        assert_eq!(PlayMode::Enhanced(chosen).enhancements(), chosen);
    }

    #[test]
    fn toggling_an_enhancement_changes_only_it() {
        let mut enhancements = Enhancements::default();
        enhancements.toggle(Enhancement::DamageNumbers);
        assert!(enhancements.is_on(Enhancement::DamageNumbers));
        assert!(enhancements.is_on(Enhancement::BattleAnimations));
        enhancements.toggle(Enhancement::BattleAnimations);
        assert!(!enhancements.is_on(Enhancement::BattleAnimations));
    }

    #[test]
    fn the_enhancements_start_as_the_original_plays_but_the_autosave() {
        assert_eq!(PlayMode::default(), PlayMode::Classic);
        assert!(Enhancements::default().battle_animations);
        assert!(!Enhancements::default().damage_numbers);
        assert!(!Enhancements::default().auto_text);
        assert!(Enhancements::default().autosave);
        assert!(Enhancements::default().weapon_reach);
    }
}
