//! Light levels — the "how well can anyone see here" layer.
//!
//! 5e's *Vision and Light* rules are three sentences long and the
//! engine implemented none of them:
//!
//!   - A **heavily obscured** area (darkness among them) "blocks vision
//!     entirely", and a creature trying to see into one "effectively
//!     suffers from the blinded condition".
//!   - A **lightly obscured** area (dim light) costs disadvantage on
//!     sight-based Perception.
//!   - **Darkvision** lets its holder "treat darkness as if it were dim
//!     light" out to a radius.
//!
//! The absence showed up in five different places as the same
//! apology. `SpecialSense::Darkvision(_)` was declared on some two
//! hundred stat blocks and read by *nothing*. The Way of Shadow monk's
//! Shadow Arts dropped two of its four spells "because the engine has
//! no light level for either to act on". The Transmuter's Stone and the
//! Diviner's Third Eye each dropped their darkvision option for the
//! same reason. Shadow of Moil collapsed its dim-light clause "since
//! the engine has no sight system". Daylight's dispel-magical-darkness
//! sentence was, in its own docstring's words, "a no-op today".
//!
//! This module is the layer all of them were missing.
//!
//! ## The three questions, and where each is answered
//!
//! **How bright is a tile?** `EncounterInstance::light_at` — the
//! brightest of the encounter's ambient level and every light source
//! that reaches the tile, floored to `Dark` by magical darkness.
//! Objective: it is the same answer for everybody.
//!
//! **How bright is a tile *to somebody*?**
//! `EncounterInstance::perceived_light` — the objective answer, then
//! upgraded one step if the viewer's darkvision reaches. Subjective, and
//! the reason the two are separate functions: a kobold and a human
//! standing shoulder to shoulder in an unlit corridor are in the same
//! darkness and only one of them is blind in it.
//!
//! **Can somebody see somebody else?**
//! `EncounterInstance::darkness_blinds`, which the existing
//! `viewer_can_see` and `compute_attack_mode` both fold in beside the
//! zone-obscurement gate they already had.
//!
//! ## Why darkness is not just another obscuring zone
//!
//! The engine already had heavy obscurement, on the zone layer, and
//! Fog Cloud and the Darkness spell both used it. Reusing it for
//! *ambient* darkness would have been wrong in two directions at once:
//!
//!   - **Obscurement is symmetric and darkness is not.** A fog bank
//!     blinds the archer outside it as surely as the target inside —
//!     the zone gate deliberately includes both endpoints and every
//!     tile between. Darkness only hides what is *in* it: a creature
//!     standing in an unlit corridor sees the torchlit room ahead
//!     perfectly well, and is itself invisible to anyone in it. The
//!     gate here reads the subject's tile and nothing else.
//!
//!   - **Darkvision beats darkness and does nothing about fog.** RAW
//!     says so outright, and `obscurement_blinds` says so in its own
//!     docstring ("a creature with darkvision in a fog bank is as blind
//!     as one without"). One predicate could not have both answers.
//!
//! Magical darkness — the Darkness spell — is the third case and gets
//! the `ZoneEffect::darkens` flag: it is a zone (so it is symmetric and
//! obscuring, and darkvision does not help) *and* it drives the tile to
//! `Dark` (so Sunlight Sensitivity lifts inside it, and so a light
//! source cannot illuminate it). That combination is exactly RAW's "a
//! creature with darkvision can't see through this darkness, and
//! nonmagical light can't illuminate it".
//!
//! ## What is deliberately not modeled
//!
//! **Dim light costs nothing in combat.** RAW's only penalty for a
//! lightly obscured area is on sight-based Perception checks, and the
//! engine has no perception check to tax. `Dim` is tracked because
//! darkvision's upgrade needs a rung between "fine" and "blind" and
//! because every light source in 5e is specified as a bright radius
//! plus a dim collar — not because standing in it does anything to a
//! d20 today.
//!
//! **Light does not spread around corners or stop at walls.** A light
//! source fills a Chebyshev radius, the same shape every burst and
//! every zone in the engine uses. Shadow-casting would need its own
//! geometry pass per source per tile, and the payoff would be a
//! darker room behind a pillar.

use crate::engine::types::Coordinate;

/// How well lit a tile is. Ordered, and the ordering is load-bearing:
/// `light_at` composes sources by taking the maximum, and darkvision
/// upgrades by stepping one rung up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LightLevel {
    /// 5e **heavily obscured**. A creature trying to see something here
    /// "effectively suffers from the blinded condition".
    Dark,
    /// 5e **lightly obscured**. Disadvantage on sight-based Perception
    /// checks, which the engine does not roll — see the module note.
    Dim,
    /// Plain sight. The engine's historical behaviour everywhere.
    Bright,
}

impl LightLevel {
    /// The brighter of two levels. The composition rule for light
    /// sources: a tile lit by a torch *and* a Daylight sphere is as
    /// bright as the better of them, never brighter and never dimmer.
    pub fn brighter_of(self, other: Self) -> Self {
        self.max(other)
    }

    /// One rung up, saturating at `Bright` — 5e darkvision's whole
    /// sentence: "you can see in dim light within the radius as if it
    /// were bright light, and in darkness as if it were dim light."
    pub fn upgraded(self) -> Self {
        match self {
            LightLevel::Dark => LightLevel::Dim,
            LightLevel::Dim | LightLevel::Bright => LightLevel::Bright,
        }
    }
}

/// The light the encounter has before anybody lights anything: the sky,
/// or the lack of one.
///
/// Four values rather than three because **sunlight is not a light
/// level**, it is a separate fact about the light that happens to be
/// there, and a whole cohort of monsters keys off it. A torchlit hall
/// and a meadow at noon are both `LightLevel::Bright`; only one of them
/// makes a drow flinch.
///
/// The default is `BrightLight`, and that choice is the engine's
/// history rather than a judgement about dungeons: before this module
/// every creature saw every other creature at every distance, which is
/// precisely a fully-lit board. Defaulting to anything else would have
/// silently rewritten every encounter in the suite. A caller that wants
/// the dark asks for it — `EncounterInstance::set_ambient_light`, or
/// `dnd-rs --dark` on the command line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AmbientLight {
    /// An unlit interior. Nothing is visible beyond what somebody is
    /// carrying and what darkvision reaches.
    Darkness,
    /// Dusk, or a hall with a few embers left. Nothing mechanical
    /// today; see the module note on `Dim`.
    DimLight,
    /// A torchlit hall. Bright, and not sunlight.
    #[default]
    BrightLight,
    /// Open ground under an unclouded sun. Bright, *and* sunlight —
    /// which is the half that matters, because it is the trigger for
    /// every Sunlight Sensitivity / Weakness / Hypersensitivity clause
    /// in the bestiary.
    Daylight,
}

impl AmbientLight {
    /// The light level this ambient contributes to every tile on the
    /// board.
    pub const fn level(self) -> LightLevel {
        match self {
            AmbientLight::Darkness => LightLevel::Dark,
            AmbientLight::DimLight => LightLevel::Dim,
            AmbientLight::BrightLight | AmbientLight::Daylight => LightLevel::Bright,
        }
    }

    /// True only for `Daylight`. The Daylight *spell* deliberately
    /// answers `false` here and so does every torch: RAW is explicit
    /// that the spell's light "is sunlight" only in the sense of being
    /// bright, and a vampire is not destroyed by a 3rd-level spell.
    pub const fn is_sunlight(self) -> bool {
        matches!(self, AmbientLight::Daylight)
    }

    /// Label for the side panel.
    pub const fn label(self) -> &'static str {
        match self {
            AmbientLight::Darkness => "darkness",
            AmbientLight::DimLight => "dim light",
            AmbientLight::BrightLight => "bright light",
            AmbientLight::Daylight => "daylight",
        }
    }

    /// Parse a command-line spelling. `None` for anything unrecognized
    /// so the caller can report it rather than guess.
    pub fn parse(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "dark" | "darkness" | "unlit" | "night" => Some(AmbientLight::Darkness),
            "dim" | "dusk" | "gloom" => Some(AmbientLight::DimLight),
            "bright" | "lit" | "torchlit" => Some(AmbientLight::BrightLight),
            "day" | "daylight" | "sun" | "sunlight" => Some(AmbientLight::Daylight),
            _ => None,
        }
    }

    /// Every spelling `parse` accepts, for the help text and for the
    /// sweep that pins the two in agreement.
    pub const NAMES: &'static [&'static str] = &["dark", "dim", "bright", "daylight"];
}

/// What a light source is attached to.
///
/// The distinction is movement, and it is the reason this is an enum
/// rather than a bare `Coordinate`: a torch goes where its bearer goes
/// and a Daylight sphere stays where it was cast. Recomputing a
/// carried source's origin from the actor table on every query — rather
/// than writing the coordinate back on every step — is what keeps the
/// two from ever disagreeing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LightAnchor {
    /// Anchored to a tile. Survives its creator's death, which is what
    /// a sphere of light hanging in the air should do.
    Fixed(Coordinate),
    /// Carried by an actor, and located wherever that actor is. A
    /// source whose bearer has left the table is dark — `origin_in`
    /// returns `None` and every reach test fails closed.
    Carried(usize),
}

/// One thing shedding light: a lit torch, a Light cantrip, a Daylight
/// sphere.
///
/// The bright / dim split is 5e's own shape — every light source in the
/// book is specified as "bright light in a N-foot radius and dim light
/// for an additional N feet" — and `dim_tiles` is that *additional*
/// collar rather than a total, so the two fields read exactly as the
/// stat line does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LightSource {
    /// Assigned by `EncounterInstance::add_light_source`; the handle a
    /// caller keeps if it intends to take the light back.
    pub id: usize,
    /// Shown in the log when the source is lit and when it gutters out.
    pub name: &'static str,
    pub anchor: LightAnchor,
    /// Radius of bright light, in tiles.
    pub bright_tiles: isize,
    /// *Additional* radius of dim light beyond `bright_tiles`, in
    /// tiles.
    pub dim_tiles: isize,
    /// Rounds left before it gutters out, or `None` for a source that
    /// burns for the whole encounter.
    ///
    /// A torch's RAW hour and the Light cantrip's RAW hour are both far
    /// longer than any fight, so both use `None` rather than a large
    /// number that would be a lie about when it ends. The field exists
    /// for sources that genuinely expire inside a fight — the Daylight
    /// spell's ten rounds.
    pub rounds_remaining: Option<u32>,
    /// The spell level this light was created at, or `0` for
    /// nonmagical flame.
    ///
    /// Read by exactly one rule, and it is a rule 5e states twice from
    /// both ends: Darkness dispels "light created by a spell of 2nd
    /// level or lower" in its area, and Daylight dispels "darkness
    /// created by a spell of 3rd level or lower" in its. Storing the
    /// level on the source is what lets the second sentence be
    /// implemented at all.
    pub spell_level: u32,
    /// True when the source *is* the creature carrying it rather than
    /// something the creature is holding — 5e's **Illumination**
    /// trait, which the azer, the magmin, the flameskull, the
    /// will-o'-wisp and the fire elemental all print in those words.
    ///
    /// Read by exactly one rule, and it is the rule that separates a
    /// glow from a torch: `drop_light_sources_carried_by` leaves a
    /// dead bearer's torch burning on the tile they fell on, which is
    /// both RAW and the more interesting board. A body that *was* the
    /// light does not do that. The azer's glow is the azer being made
    /// of fire, and when the fire goes out the corridor goes dark —
    /// which is the whole tactical shape of killing one.
    ///
    /// Only meaningful on a `LightAnchor::Carried` source. A fixed
    /// innate light is a contradiction (nothing is carrying it) and the
    /// drop routine never looks at one.
    pub innate: bool,
}

impl LightSource {
    /// Where this source is right now, given the actor table, or `None`
    /// if it is carried by somebody who is no longer in it.
    ///
    /// The anchor *tile*. Callers measuring a distance from a carried
    /// source want the bearer's whole body instead — see
    /// `EncounterInstance::distance_from_light` — because a
    /// Huge creature's torch is not held in the corner of its space.
    /// This accessor is for the callers that want the identity of the
    /// tile rather than a distance from it.
    pub fn origin_in(
        &self,
        actors: &std::collections::HashMap<usize, crate::actors::actor_template::ActorInstance>,
    ) -> Option<Coordinate> {
        match self.anchor {
            LightAnchor::Fixed(c) => Some(c),
            LightAnchor::Carried(id) => actors.get(&id).map(|a| a.location()),
        }
    }

    /// The light this source contributes at `dist` tiles from it.
    /// `Dark` — i.e. nothing — beyond the dim collar.
    ///
    /// Takes a distance rather than measuring one, because *how* the
    /// distance is measured depends on what the source is attached to
    /// and only the encounter knows: a fixed sphere is a point, and a
    /// carried torch is wherever its bearer's body is. See the module
    /// note on why light does not respect walls.
    ///
    /// **A source with no bright radius sheds no bright light, not even
    /// on the tile it sits on.** The `> 0` guard is the whole of that
    /// sentence, and without it `dist <= self.bright_tiles` is true at
    /// `dist == 0` for a source declaring `bright_tiles: 0` — so a
    /// spell whose entire RAW text is "sheds *dim* light in a 10-foot
    /// radius" would put a single tile of bright light at its centre.
    /// Nothing in the engine declared a zero bright radius until
    /// Dancing Lights did, which is why the off-by-one sat here
    /// unexposed: every torch, glow and sphere has a bright core, and
    /// for all of them the guard changes nothing.
    ///
    /// The centre tile is exactly where it would have mattered. A
    /// caster throws four motes down a corridor to find out what is
    /// standing at the end of it, and the one tile that would have read
    /// `Bright` is the tile the thing is standing on — so the cantrip
    /// would have handed out the clean shot that separates it from
    /// Daylight.
    pub fn level_at_distance(&self, dist: isize) -> LightLevel {
        if self.bright_tiles > 0 && dist <= self.bright_tiles {
            LightLevel::Bright
        } else if dist <= self.bright_tiles + self.dim_tiles {
            LightLevel::Dim
        } else {
            LightLevel::Dark
        }
    }
}

/// How badly a creature reacts to standing in sunlight.
///
/// Three tiers because 5e writes three, under three different names,
/// and the differences are not flavour:
///
///   - **Sunlight Sensitivity** (kobold, drow) — "disadvantage on
///     attack rolls, as well as on Wisdom (Perception) checks that rely
///     on sight."
///   - **Sunlight Weakness** (shadow) — "disadvantage on attack rolls,
///     ability checks, and saving throws." One more clause, and it is
///     the clause that matters: a shadow in the sun fails the saves it
///     would otherwise make.
///   - **Sunlight Hypersensitivity** (vampire, vampire spawn) — the
///     Weakness clauses plus "the vampire takes 20 radiant damage when
///     it starts its turn in sunlight."
///
/// Modeled as one `Option`-shaped template field rather than three
/// booleans so the tiers cannot be set in contradictory combinations,
/// and so the two predicates below are the only places that know which
/// tier implies which clause.
///
/// The perception half of all three is dropped throughout: the engine
/// rolls no Perception checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SunlightFrailty {
    /// Kobold, drow. Attack rolls only.
    Sensitivity,
    /// Shadow. Attack rolls, ability checks, and saving throws.
    Weakness,
    /// Vampire, vampire spawn. Weakness, plus 20 radiant at the start
    /// of each turn spent in the sun.
    Hypersensitivity,
}

impl SunlightFrailty {
    /// True for every tier: all three disadvantage attack rolls made in
    /// sunlight.
    pub const fn disadvantages_attacks(self) -> bool {
        true
    }

    /// True for the two tiers whose text reaches saving throws.
    pub const fn disadvantages_saves(self) -> bool {
        matches!(
            self,
            SunlightFrailty::Weakness | SunlightFrailty::Hypersensitivity
        )
    }

    /// Radiant damage taken at the start of a turn begun in sunlight.
    /// `0` for the tiers that only suffer penalties.
    pub const fn start_of_turn_radiant(self) -> u32 {
        match self {
            SunlightFrailty::Sensitivity | SunlightFrailty::Weakness => 0,
            SunlightFrailty::Hypersensitivity => 20,
        }
    }

    /// Name for the log line when the damage lands.
    pub const fn label(self) -> &'static str {
        match self {
            SunlightFrailty::Sensitivity => "sunlight sensitivity",
            SunlightFrailty::Weakness => "sunlight weakness",
            SunlightFrailty::Hypersensitivity => "sunlight hypersensitivity",
        }
    }
}

/// Bright radius of a torch, a Light cantrip, and every other mundane
/// flame in the equipment list: 20 ft = 8 tiles on the 2.5-ft grid.
/// The dim collar is another 20 ft, which is why both constants are the
/// same number and are nonetheless two constants — RAW specifies them
/// separately and a future lantern changes only one.
pub const TORCH_BRIGHT_TILES: isize = 8;
pub const TORCH_DIM_TILES: isize = 8;

/// 5e **Illumination**, the ten-foot flavour: "sheds bright light in a
/// 10-foot radius and dim light for an additional 10 feet." The azer's
/// and the magmin's, and the most common printing of the trait.
///
/// Four tiles on the 2.5-ft grid — half a torch, which is the right
/// relationship: a creature that glows is a worse lamp than a lamp, and
/// the interesting half of the trait is that it cannot be put away.
pub const GLOW_BRIGHT_TILES: isize = 4;
pub const GLOW_DIM_TILES: isize = 4;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ordering_is_dark_to_bright() {
        assert!(LightLevel::Dark < LightLevel::Dim);
        assert!(LightLevel::Dim < LightLevel::Bright);
        assert_eq!(
            LightLevel::Dark.brighter_of(LightLevel::Bright),
            LightLevel::Bright
        );
    }

    /// Darkvision's upgrade is one rung and saturates — a creature with
    /// darkvision in bright light does not see "extra bright", which
    /// matters because nothing downstream should be able to read a
    /// fourth level off the enum.
    #[test]
    fn darkvision_upgrades_one_rung_and_saturates() {
        assert_eq!(LightLevel::Dark.upgraded(), LightLevel::Dim);
        assert_eq!(LightLevel::Dim.upgraded(), LightLevel::Bright);
        assert_eq!(LightLevel::Bright.upgraded(), LightLevel::Bright);
    }

    /// A source with a bright core lights it, and the collar beyond it
    /// dim — the ordinary shape every torch and sphere in the engine
    /// has.
    #[test]
    fn a_source_lights_its_bright_core_and_a_dim_collar_beyond_it() {
        let torch = LightSource {
            id: 0,
            name: "torch",
            anchor: LightAnchor::Fixed(Coordinate::new(0, 0)),
            bright_tiles: 8,
            dim_tiles: 8,
            rounds_remaining: None,
            spell_level: 0,
            innate: false,
        };
        assert_eq!(torch.level_at_distance(0), LightLevel::Bright);
        assert_eq!(torch.level_at_distance(8), LightLevel::Bright);
        assert_eq!(torch.level_at_distance(9), LightLevel::Dim);
        assert_eq!(torch.level_at_distance(16), LightLevel::Dim);
        assert_eq!(torch.level_at_distance(17), LightLevel::Dark);
    }

    /// A source with no bright radius sheds no bright light *anywhere*,
    /// including the tile it sits on.
    ///
    /// The centre tile is the whole test. `dist <= bright_tiles` is
    /// true at `dist == 0` for `bright_tiles: 0`, so the obvious
    /// implementation gives a dim-only source a one-tile bright core —
    /// and the one tile it would appear on is the tile whatever the
    /// caster is looking at is standing on. Dancing Lights is the
    /// engine's only zero-bright source and would have been the only
    /// thing to notice.
    #[test]
    fn a_source_with_no_bright_radius_is_dim_at_its_own_centre() {
        let motes = LightSource {
            id: 0,
            name: "dancing lights",
            anchor: LightAnchor::Fixed(Coordinate::new(0, 0)),
            bright_tiles: 0,
            dim_tiles: 4,
            rounds_remaining: Some(10),
            spell_level: 0,
            innate: false,
        };
        assert_eq!(
            motes.level_at_distance(0),
            LightLevel::Dim,
            "the tile the motes hang over is dim, not bright"
        );
        assert_eq!(motes.level_at_distance(4), LightLevel::Dim);
        assert_eq!(motes.level_at_distance(5), LightLevel::Dark);
    }

    /// The default is the fully-lit board the engine behaved as before
    /// this module existed. Pinned because changing it silently
    /// rewrites every encounter in the suite.
    #[test]
    fn the_default_ambient_is_a_lit_board_that_is_not_sunlight() {
        let a = AmbientLight::default();
        assert_eq!(a, AmbientLight::BrightLight);
        assert_eq!(a.level(), LightLevel::Bright);
        assert!(!a.is_sunlight());
    }

    /// Only the sky is sunlight. Pinned separately from the level
    /// because "bright" and "sunlit" are exactly the two things this
    /// enum exists to keep apart.
    #[test]
    fn only_daylight_is_sunlight_and_it_is_also_bright() {
        assert!(AmbientLight::Daylight.is_sunlight());
        assert_eq!(AmbientLight::Daylight.level(), LightLevel::Bright);
        for a in [
            AmbientLight::Darkness,
            AmbientLight::DimLight,
            AmbientLight::BrightLight,
        ] {
            assert!(!a.is_sunlight(), "{:?}", a);
        }
    }

    /// Every name the help text advertises actually parses, and parses
    /// to a distinct ambient. Without this the listing and the parser
    /// can drift apart, which is the failure mode where the help tells
    /// a player to type something the parser refuses.
    #[test]
    fn every_advertised_ambient_name_parses_to_its_own_value() {
        let mut seen = Vec::new();
        for name in AmbientLight::NAMES {
            let parsed = AmbientLight::parse(name)
                .unwrap_or_else(|| panic!("advertised name {:?} does not parse", name));
            assert!(!seen.contains(&parsed), "{:?} is advertised twice", parsed);
            seen.push(parsed);
        }
        assert_eq!(seen.len(), 4, "one name per ambient: {:?}", seen);
    }

    #[test]
    fn parsing_is_case_insensitive_and_refuses_nonsense() {
        assert_eq!(AmbientLight::parse("DARK"), Some(AmbientLight::Darkness));
        assert_eq!(AmbientLight::parse("Daylight"), Some(AmbientLight::Daylight));
        assert_eq!(AmbientLight::parse("puce"), None);
    }

    /// The bright radius is inclusive, the dim collar is *additional*,
    /// and past both there is nothing. The middle clause is the one
    /// worth pinning: reading `dim_tiles` as a total instead of a
    /// collar halves every light source in the game.
    #[test]
    fn a_source_lights_a_bright_core_then_a_dim_collar_then_nothing() {
        let src = LightSource {
            id: 0,
            name: "torch",
            anchor: LightAnchor::Fixed(Coordinate::new(0, 0)),
            bright_tiles: 2,
            dim_tiles: 3,
            rounds_remaining: None,
            spell_level: 0,
            innate: false,
        };
        for (dist, expected) in [
            (0, LightLevel::Bright),
            (2, LightLevel::Bright),
            (3, LightLevel::Dim),
            (5, LightLevel::Dim),
            (6, LightLevel::Dark),
        ] {
            assert_eq!(
                src.level_at_distance(dist),
                expected,
                "at distance {}",
                dist
            );
        }
    }

    /// The tier table, pinned clause by clause. Every tier taxes
    /// attacks; only the top two reach saves; only the top one burns.
    #[test]
    fn the_frailty_tiers_add_clauses_rather_than_replace_them() {
        use SunlightFrailty::*;
        for f in [Sensitivity, Weakness, Hypersensitivity] {
            assert!(f.disadvantages_attacks(), "{:?}", f);
        }
        assert!(!Sensitivity.disadvantages_saves());
        assert!(Weakness.disadvantages_saves());
        assert!(Hypersensitivity.disadvantages_saves());
        assert_eq!(Sensitivity.start_of_turn_radiant(), 0);
        assert_eq!(Weakness.start_of_turn_radiant(), 0);
        assert_eq!(Hypersensitivity.start_of_turn_radiant(), 20);
    }
}
