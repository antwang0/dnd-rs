//! **Weather** — SRD 5.2's *Environmental Effects*, or the half of them
//! a fight can see.
//!
//! The section prints nine entries and seven of them are measured in
//! hours: deep water, extreme cold, extreme heat, frigid water, high
//! altitude are all "at the end of each hour, save or gain a level of
//! Exhaustion", and thin ice is a weight budget for a party crossing a
//! lake. None of those has a surface on a board whose longest fight is
//! twenty rounds — two minutes.
//!
//! Two do. **Strong Wind** and **Heavy Precipitation** are written
//! entirely in the vocabulary of a round: an attack roll, a flying
//! creature's turn, a light source, an obscurement. They are what this
//! module is.
//!
//! ## Why a board-level enum
//!
//! For the reason `lighting::AmbientLight` is one, and this is its
//! sibling in every respect: the weather is a fact about the *place*
//! rather than about anybody standing in it, it does not change during a
//! fight unless somebody changes it, and it is the sort of thing a
//! player wants to set from the command line before the first round.
//! `AmbientLight` even shares the shape — a `label` for the panel, a
//! `parse` for the argument, a `NAMES` list that the two are pinned
//! against each other on.
//!
//! Not a zone, though the zone system could carry it. RAW's own framing
//! is "an area of heavy rain", which sounds like a zone and is not one:
//! the area in question is the sky over the whole encounter, and a fog
//! bank that happens to be raining is a different thing the engine
//! already has. Weather that covered part of the board would need a
//! shape, a boundary and a per-tile query on the hottest path in the
//! engine, to model a distinction no fight in the book draws.
//!
//! ## What each entry does, and what it does not
//!
//! | entry | clause | where it lives |
//! |-------|--------|----------------|
//! | Strong Wind | Disadvantage on ranged weapon attacks | `attack::resolve_attack_outcome` |
//! | Strong Wind | a flier must land at the end of its turn | `EncounterInstance::ground_fliers_in_the_wind` |
//! | Strong Wind | extinguishes open flames | `EncounterInstance::snuff_open_flames` |
//! | Heavy Precipitation | everything is Lightly Obscured | `EncounterInstance::light_at` |
//! | Heavy Precipitation | extinguishes open flames | `EncounterInstance::snuff_open_flames` |
//!
//! Three clauses are absent and are named here rather than dropped:
//!
//!   - **"Disperses fog."** The engine's fog is `Fog Cloud`, a
//!     concentration zone, and dispersing it would mean ending another
//!     creature's spell from the weather — a lane nothing else in the
//!     engine has and one that would need a rule for whether the caster
//!     may simply re-cast it into the same wind next turn. RAW does not
//!     say.
//!   - **"Disadvantage on Wisdom (Perception) checks"** (both entries).
//!     Heavy Precipitation's obscurement already taxes exactly the two
//!     Perception checks this engine rolls, through
//!     `DIM_LIGHT_PERCEPTION_PENALTY`; a second penalty on top of it
//!     would be the same rule charged twice. Strong Wind's version is
//!     printed as a *sandstorm* clause, conditional on a desert.
//!   - **"A strong wind in a desert can create a sandstorm."** The board
//!     has terrain but no biome, so there is nothing to ask.

/// What the sky is doing to the fight.
///
/// Three values rather than a `bool` per clause because SRD 5.2 writes
/// them as three named conditions with overlapping consequences, and
/// because a fight has one weather at a time. Wind and rain both put
/// torches out; only wind grounds fliers and taxes archers; only rain
/// dims the board.
///
/// The default is `Calm`, and that choice is the engine's history rather
/// than a claim about the world: before this module every fight was
/// fought in still air, and defaulting to anything else would have
/// silently rewritten every encounter in the suite. A caller that wants
/// weather asks for it — `EncounterInstance::set_weather`, or
/// `dnd-rs --wind` on the command line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Weather {
    /// Still air. Nothing here does anything, which is the point.
    #[default]
    Calm,
    /// SRD 5.2 **Strong Wind**: *"Strong wind imposes Disadvantage on
    /// ranged attack rolls with weapons. It also extinguishes open
    /// flames and disperses fog. A flying creature in a strong wind must
    /// land at the end of its turn or fall."*
    StrongWind,
    /// SRD 5.2 **Heavy Precipitation**: *"Everything within an area of
    /// heavy rain or heavy snowfall is Lightly Obscured, and creatures in
    /// the area have Disadvantage on all Wisdom (Perception) checks.
    /// Heavy rain also extinguishes open flames."*
    HeavyPrecipitation,
}

impl Weather {
    /// True while the sky is taxing ranged *weapon* attacks.
    ///
    /// The qualifier is RAW's and it is load-bearing: "ranged attack
    /// rolls **with weapons**" leaves spell attacks alone, so a wind that
    /// shuts an archer down does nothing to a Fire Bolt. That is why the
    /// clause is read at the weapon chokepoint, which knows the
    /// difference, rather than in the shared `attack_mode_tally`, which
    /// does not.
    pub const fn taxes_ranged_weapons(self) -> bool {
        matches!(self, Weather::StrongWind)
    }

    /// True while a creature in the air has to come down at the end of
    /// its turn. See `EncounterInstance::ground_fliers_in_the_wind` for
    /// what "come down" resolves to and why it is not a fall.
    pub const fn grounds_fliers(self) -> bool {
        matches!(self, Weather::StrongWind)
    }

    /// True while an open flame cannot stay lit — both entries, because
    /// RAW gives both the clause: the wind "extinguishes open flames"
    /// and heavy rain "also extinguishes open flames".
    pub const fn snuffs_open_flames(self) -> bool {
        matches!(self, Weather::StrongWind | Weather::HeavyPrecipitation)
    }

    /// True while the whole board is Lightly Obscured.
    ///
    /// The engine spells "lightly obscured" `LightLevel::Dim`, so this
    /// is a *cap* on the light rather than a floor on the dark: rain
    /// makes a sunlit field gloomy and does nothing whatsoever to a
    /// cellar that was already black.
    pub const fn obscures(self) -> bool {
        matches!(self, Weather::HeavyPrecipitation)
    }

    /// Label for the side panel.
    pub const fn label(self) -> &'static str {
        match self {
            Weather::Calm => "calm",
            Weather::StrongWind => "strong wind",
            Weather::HeavyPrecipitation => "heavy rain",
        }
    }

    /// Parse a command-line spelling. `None` for anything unrecognized
    /// so the caller can report it rather than guess.
    pub fn parse(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "calm" | "still" | "fair" => Some(Weather::Calm),
            "wind" | "windy" | "gale" | "strongwind" => Some(Weather::StrongWind),
            "rain" | "rainy" | "storm" | "snow" | "downpour" => {
                Some(Weather::HeavyPrecipitation)
            }
            _ => None,
        }
    }

    /// Every spelling `parse` accepts as a *flag*, for the help text and
    /// for the sweep that pins the two in agreement. `calm` is absent
    /// deliberately — it is the default, so a flag for it would be a
    /// flag that does nothing.
    pub const NAMES: &'static [&'static str] = &["wind", "rain"];
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Both entries put torches out; only one grounds fliers, only one
    /// taxes archers, only one dims the board. The overlap is why this
    /// is one enum with four predicates rather than two booleans.
    #[test]
    fn the_two_entries_share_one_clause_and_differ_on_the_rest() {
        assert!(Weather::StrongWind.snuffs_open_flames());
        assert!(Weather::HeavyPrecipitation.snuffs_open_flames());
        assert!(!Weather::Calm.snuffs_open_flames());

        assert!(Weather::StrongWind.grounds_fliers());
        assert!(!Weather::HeavyPrecipitation.grounds_fliers());

        assert!(Weather::StrongWind.taxes_ranged_weapons());
        assert!(!Weather::HeavyPrecipitation.taxes_ranged_weapons());

        assert!(Weather::HeavyPrecipitation.obscures());
        assert!(!Weather::StrongWind.obscures());
    }

    /// Calm does nothing at all, which is what makes it safe as the
    /// default for every encounter that predates this module.
    #[test]
    fn calm_weather_has_no_clauses() {
        let calm = Weather::Calm;
        assert!(!calm.taxes_ranged_weapons());
        assert!(!calm.grounds_fliers());
        assert!(!calm.snuffs_open_flames());
        assert!(!calm.obscures());
    }

    /// Every flag name the help text advertises parses back to a
    /// weather, and to one that is not the default — a flag for `calm`
    /// would be a flag that does nothing.
    #[test]
    fn every_advertised_flag_parses_to_something_worth_asking_for() {
        for name in Weather::NAMES {
            let parsed = Weather::parse(name)
                .unwrap_or_else(|| panic!("{name} is advertised but does not parse"));
            assert_ne!(parsed, Weather::Calm, "{name} asks for the default");
        }
        assert_eq!(Weather::parse("GALE"), Some(Weather::StrongWind));
        assert_eq!(Weather::parse("puce"), None);
    }
}
