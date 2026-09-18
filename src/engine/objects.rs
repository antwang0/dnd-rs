//! **Breaking Objects** — SRD 5.2's rule for hitting a thing that is
//! not a creature, and the wall spells it was written for.
//!
//! > Objects can be harmed by attacks and by some spells, using the
//! > rules below.
//! >
//! > **Armor Class.** The Object Armor Class table suggests ACs for
//! > various substances.
//! >
//! > **Hit Points.** An object is destroyed when it has 0 Hit Points.
//! >
//! > **Damage Types and Objects.** Objects have Immunity to Poison and
//! > Psychic damage.
//! >
//! > **No Ability Scores.** An object lacks ability scores unless a rule
//! > assigns scores to the object. Without ability scores, an object
//! > can't make ability checks, and it fails all saving throws.
//!
//! ## What is an object here, and what is not
//!
//! Only **conjured terrain** — see [`crate::engine::conjured_terrain`].
//! A Wall of Ice and a Wall of Stone each print their own AC, their own
//! hit points and their own damage modifiers in the spell's text, which
//! is the book asking for exactly this, and a wall the party cannot get
//! through is a different spell from a wall they can chop through in
//! three rounds.
//!
//! The dungeon's own stone is not an object, and that is a deliberate
//! narrowing rather than an omission. RAW would let a party tunnel out
//! through any wall in the map given enough rounds; the engine's walls
//! are generated scenery whose job is to shape a fight, and a
//! `TerrainType::Wall` that could be removed would make every corridor
//! optional. The line is ownership, which is the same line
//! `conjured_terrain` already draws against `zones`: a wall somebody
//! *cast* is a thing they put there, and a thing somebody put there can
//! be taken away.
//!
//! Carried objects — a shield, a rope, a potion bottle — are not here
//! either. The engine has an inventory but no notion of a held object
//! occupying space, so there is nothing for an attack to be aimed at.
//!
//! ## Two ways through a wall
//!
//! Both are RAW's *"by attacks and by some spells"*, and the engine
//! needs both because they belong to different parties:
//!
//!   - **An area that covers the tile.**
//!     `EncounterInstance::damage_objects_in_area`, queued by the same
//!     helper every save-for-half burst in the engine resolves through.
//!     A Fireball dropped on a Wall of Ice melts a hole in it, which is
//!     what the wall's *Vulnerability to Fire* is printed for; a Cone
//!     of Cold does nothing at all to it, which is what its *Immunity
//!     to Cold* is printed for.
//!   - **A swing.** [`crate::actions::default_actions::Sunder`], an
//!     Action anybody can take against a breakable tile they are
//!     standing next to. Without it a party with no area damage meets a
//!     Wall of Stone and simply stops, which is not a tactical problem
//!     so much as the absence of one.
//!
//! The object never saves, however it is hit: *"an object … fails all
//! saving throws"*, so the full rolled damage lands on it rather than
//! the halved amount its neighbours took.

use crate::engine::types::{DamageModifier, DamageType};

/// SRD 5.2's **Object Armor Class** table, whole.
///
/// | AC | Substance | AC | Substance |
/// |----|-----------|----|-----------|
/// | 11 | Cloth, paper, rope | 19 | Iron, steel |
/// | 13 | Crystal, glass, ice | 21 | Mithral |
/// | 15 | Wood | 23 | Adamantine |
/// | 17 | Stone | | |
///
/// Every row is here and two are used, which is the right way round for
/// a printed table: the numbers are a *ladder*, and a reader deciding
/// what a new object should be needs to see where the rungs are. The
/// two in use are not even drawn from it — Wall of Ice prints `AC 12`
/// and Wall of Stone prints `AC 15`, each in its own spell text, and
/// both differ from the substance the table would have given them (ice
/// 13, stone 17). A conjured wall is thinner than the real thing, which
/// is what the spell is saying by printing its own number.
pub const OBJECT_AC_CLOTH: u32 = 11;
pub const OBJECT_AC_GLASS: u32 = 13;
pub const OBJECT_AC_WOOD: u32 = 15;
pub const OBJECT_AC_STONE: u32 = 17;
pub const OBJECT_AC_IRON: u32 = 19;
pub const OBJECT_AC_MITHRAL: u32 = 21;
pub const OBJECT_AC_ADAMANTINE: u32 = 23;

/// The two damage types nothing inanimate can be hurt by — SRD 5.2's
/// *"Objects have Immunity to Poison and Psychic damage"*.
///
/// Folded into every profile at lookup time rather than written into
/// each one, because it is a fact about objects rather than about any
/// particular object: a profile that had to remember it is a profile
/// that can forget it.
pub const OBJECT_IMMUNITIES: &[DamageType] = &[DamageType::Poison, DamageType::Psychic];

/// How much punishment one tile of a breakable thing takes, and what it
/// takes it from.
///
/// A `&'static` shared by every tile of every copy of the thing, in the
/// same way a `CreatureTemplate` is shared by every copy of a goblin.
/// The per-tile *state* — how much of the hit points are left — lives on
/// the patch, because that is what differs between two walls of ice on
/// the same board.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectProfile {
    /// What the log calls it when a blow lands: "the wall of ice".
    pub label: &'static str,
    /// The number an attack roll has to beat. From the spell's own text
    /// where it prints one, and from [the substance table](self)
    /// otherwise.
    pub ac: u32,
    /// Hit points for **one tile**, which is not the unit RAW counts in.
    ///
    /// The book prices a wall by its 10-foot section — `30 Hit Points
    /// per 10-foot section` for ice, `30 Hit Points per inch of
    /// thickness` across a six-inch panel for stone — and this engine's
    /// grid is 2.5 feet, so a section is four tiles of frontage and a
    /// tile is a quarter of one. Each profile below shows the division
    /// it made.
    ///
    /// Per tile rather than per patch, because a hole is the whole
    /// point: RAW's *"reducing a panel to 0 Hit Points destroys it and
    /// leaves behind a hole"* is a sentence about part of a wall, and a
    /// wall with one pool of hit points would vanish all at once.
    pub hp_per_tile: u32,
    /// The one damage type the thing is made worse by, if any — ice's
    /// fire.
    pub vulnerable_to: Option<DamageType>,
    /// Damage types that do nothing to it, **on top of**
    /// [`OBJECT_IMMUNITIES`]. Ice's cold; stone's nothing.
    pub immune_to: &'static [DamageType],
}

impl ObjectProfile {
    /// How this thing scales an incoming instance of damage.
    ///
    /// `None` for the ordinary case, which is what
    /// `ActorInstance::damage_modifier` answers for a creature with no
    /// entry, so the two read the same way at the two call sites.
    pub fn damage_modifier(&self, dt: DamageType) -> Option<DamageModifier> {
        if OBJECT_IMMUNITIES.contains(&dt) || self.immune_to.contains(&dt) {
            return Some(DamageModifier::Immunity);
        }
        if self.vulnerable_to == Some(dt) {
            return Some(DamageModifier::Vulnerability);
        }
        None
    }

    /// What `amount` of `dt` actually takes off this thing's hit points.
    ///
    /// The object-side twin of `ActorInstance::effective_damage`, and
    /// the same arithmetic: immunity zeroes, vulnerability doubles,
    /// resistance halves. Nothing in the book gives an object
    /// Resistance, so the halving arm is here for the shape rather than
    /// for a caller.
    pub fn effective_damage(&self, amount: u32, dt: DamageType) -> u32 {
        match self.damage_modifier(dt) {
            Some(DamageModifier::Immunity) => 0,
            Some(DamageModifier::Vulnerability) => amount.saturating_mul(2),
            Some(DamageModifier::Resistance) => amount / 2,
            // An object cannot be healed by being hit, and nothing in
            // the book tries. Treated as no modifier rather than as an
            // absorption, so a hypothetical future profile that set it
            // by mistake still loses hit points.
            Some(DamageModifier::Absorption) | None => amount,
        }
    }
}

/// What one instance of damage did to a breakable tile.
///
/// Four arms rather than a `bool`, because the three failures read
/// differently to whoever asked and two of them are not failures at
/// all: a Cone of Cold that `Shrugged` off a wall of ice told the
/// caster something about the wall, and a swing that found
/// `NothingThere` told them they were aiming at floor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectDamageOutcome {
    /// Nothing breakable on the tile — open floor, the dungeon's own
    /// stone, or a Wall of Force.
    NothingThere,
    /// Immunity swallowed it whole. RAW's Poison and Psychic, and the
    /// ice wall's Cold.
    Shrugged,
    /// Hit points came off and some are left.
    Damaged { remaining: u32 },
    /// The last of them came off: RAW's *"reducing a panel to 0 Hit
    /// Points destroys it and leaves behind a hole"*, and the tile is
    /// back to whatever was underneath.
    Breached,
}

/// **Wall of Ice**, SRD 5.2: *"The wall is an object that can be damaged
/// and thus breached. It has AC 12 and 30 Hit Points per 10-foot
/// section, and it has Immunity to Cold, Poison, and Psychic damage and
/// Vulnerability to Fire damage."*
///
/// `30 / 4 = 7.5`, rounded up to **8** — see [`ObjectProfile::hp_per_tile`]
/// for why a tile is a quarter of a section. Rounded up rather than
/// down because half a point of a section's resilience is worth less
/// than the guarantee that a wall never falls to less than the book
/// prints.
///
/// The numbers are the spell, and the arithmetic is worth reading once.
/// A Fireball averages 28, doubled to 56 by the vulnerability, which
/// clears eight-per-tile on every tile it covers: one third-level slot
/// opens a hole four tiles wide in a sixth-level wall. That is RAW —
/// `28 × 2 = 56` against a section's 30 — and it is the whole reason
/// the wall prints a fire line at all. A Cone of Cold aimed at the same
/// wall does nothing whatsoever.
pub static WALL_OF_ICE_PROFILE: ObjectProfile = ObjectProfile {
    label: "wall of ice",
    ac: 12,
    hp_per_tile: 8,
    vulnerable_to: Some(DamageType::Fire),
    // Poison and Psychic ride `OBJECT_IMMUNITIES`; the spell's own line
    // adds cold, which is the interesting half — the wall cannot be
    // taken down by the element it is made of.
    immune_to: &[DamageType::Cold],
};

/// **Wall of Stone**, SRD 5.2: *"The wall is an object made of stone
/// that can be damaged and thus breached. Each panel has AC 15 and 30
/// Hit Points per inch of thickness, and it has Immunity to Poison and
/// Psychic damage."*
///
/// The wall is six inches thick, so a panel is `6 × 30 = 180` hit
/// points, and `180 / 4 = 45` per tile.
///
/// Forty-five is a lot and is meant to be: a fighter with a greataxe
/// takes four rounds over one tile, and a party that wants through a
/// Wall of Stone in a hurry needs to bring something bigger than a
/// weapon. That is the difference between the two walls, and it is the
/// two spell levels between them — the ice wall is a delay and the
/// stone wall is a decision.
///
/// No vulnerability: stone is stone, and the spell prints none.
pub static WALL_OF_STONE_PROFILE: ObjectProfile = ObjectProfile {
    label: "wall of stone",
    ac: 15,
    hp_per_tile: 45,
    vulnerable_to: None,
    immune_to: &[],
};

#[cfg(test)]
mod tests {
    use super::*;

    /// The universal immunity is the profile's, not each wall's — a
    /// profile that had to remember Poison and Psychic is a profile
    /// that can forget them, and the failure would be a wall quietly
    /// dissolving under a Cloudkill.
    #[test]
    fn nothing_inanimate_can_be_poisoned_or_driven_mad() {
        for profile in [&WALL_OF_ICE_PROFILE, &WALL_OF_STONE_PROFILE] {
            for dt in OBJECT_IMMUNITIES {
                assert_eq!(
                    profile.damage_modifier(*dt),
                    Some(DamageModifier::Immunity),
                    "{} should shrug off {:?}",
                    profile.label,
                    dt
                );
                assert_eq!(profile.effective_damage(100, *dt), 0, "{}", profile.label);
            }
        }
    }

    /// The two lines the ice wall's spell text prints about damage, and
    /// the reason to aim a Fireball at it rather than a Cone of Cold.
    #[test]
    fn the_ice_wall_melts_and_cannot_be_frozen() {
        assert_eq!(
            WALL_OF_ICE_PROFILE.effective_damage(28, DamageType::Fire),
            56,
            "RAW gives it Vulnerability to Fire"
        );
        assert_eq!(WALL_OF_ICE_PROFILE.effective_damage(28, DamageType::Cold), 0);
        // And an ordinary swing is an ordinary swing.
        assert_eq!(
            WALL_OF_ICE_PROFILE.effective_damage(9, DamageType::Slashing),
            9
        );
    }

    /// One Fireball through one wall, in the arithmetic the profile's
    /// docstring claims. If `hp_per_tile` ever drifts, this is the
    /// sentence that stops being true.
    ///
    /// The comparison the *ice* wall is about is the vulnerability: an
    /// average Fireball clears more than three tiles' worth of it. The
    /// comparison the *stone* wall is about is the absence of one — the
    /// same spell that would breach four tiles of ice leaves a tile of
    /// stone standing, and only because it is not doubled.
    #[test]
    fn one_fireball_opens_a_hole_in_a_wall_of_ice() {
        const AVERAGE_FIREBALL: u32 = 28;
        let ice = WALL_OF_ICE_PROFILE.effective_damage(AVERAGE_FIREBALL, DamageType::Fire);
        assert_eq!(ice, 56, "8d6 doubled by the wall's own Vulnerability");
        assert!(
            ice >= WALL_OF_ICE_PROFILE.hp_per_tile,
            "an average Fireball should clear a tile of ice"
        );
        let stone = WALL_OF_STONE_PROFILE.effective_damage(AVERAGE_FIREBALL, DamageType::Fire);
        assert_eq!(stone, AVERAGE_FIREBALL, "stone has no fire line");
        assert!(
            stone < WALL_OF_STONE_PROFILE.hp_per_tile,
            "and should not clear a tile of stone"
        );
    }

    /// The substance table is a ladder and has to climb. A transposed
    /// pair would be invisible — every number in it is plausible — so
    /// the ordering is the assertion.
    #[test]
    fn the_substance_table_climbs() {
        let ladder = [
            OBJECT_AC_CLOTH,
            OBJECT_AC_GLASS,
            OBJECT_AC_WOOD,
            OBJECT_AC_STONE,
            OBJECT_AC_IRON,
            OBJECT_AC_MITHRAL,
            OBJECT_AC_ADAMANTINE,
        ];
        assert!(
            ladder.windows(2).all(|w| w[0] < w[1]),
            "SRD 5.2's Object Armor Class table runs 11..23 by twos"
        );
        // And the two walls price themselves below the substance they
        // are made of, which is the spells' own doing — see the table's
        // docstring.
        assert!(WALL_OF_ICE_PROFILE.ac < OBJECT_AC_GLASS);
        assert!(WALL_OF_STONE_PROFILE.ac < OBJECT_AC_STONE);
    }
}
