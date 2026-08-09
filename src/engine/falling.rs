//! 5e **Falling** — the hazard that turns "the wizard stopped
//! concentrating" into a number.
//!
//! SRD: *"The creature takes 1d6 bludgeoning damage for every 10 feet it
//! fell, to a maximum of 20d6. The creature lands prone, unless it
//! avoids taking damage from the fall."*
//!
//! The engine had every ingredient of that sentence and none of the
//! sentence. Three separate effects put an actor sixty feet in the air
//! (`Fly`, `Investiture of Wind`, `Otherworldly Guise`, gathered on
//! [`MAGICAL_FLIGHT_CONDITIONS`]), four separate things could take the
//! flight away mid-fight — a broken concentration save, a Dispel Magic,
//! a lapsed timer, a dropped pair of winged boots — and all four
//! resolved as the creature simply standing on the floor again, with no
//! damage, no prone, and nothing in the log. Casting Fly was
//! consequence-free, and the counterplay to a flier was worth nothing
//! beyond the tempo of the spell that removed it.
//!
//! # The altitude model
//!
//! An actor carries one number, [`ActorInstance::altitude_ft`], and the
//! board is otherwise flat. That number is not a third coordinate: the
//! map is 2D and stays 2D, nothing pathfinds through the air, and no
//! attack checks it for range. It is a *height above the floor*, and its
//! only job is to be the multiplier on the damage owed when the thing
//! holding the actor up lets go.
//!
//! The number moves under exactly two rules, both enforced by
//! `EncounterInstance::reconcile_altitudes`, which runs at the same
//! three chokepoints `reconcile_footprints` does:
//!
//!   - An actor whose flight is **supported** — any condition on
//!     `MAGICAL_FLIGHT_CONDITIONS`, read through
//!     `ActorInstance::has_magical_flight` — rises to
//!     [`FLIGHT_ALTITUDE_FT`] and stays there.
//!   - An actor whose supported altitude drops below where it currently
//!     is **falls the difference**, which is the whole of this module.
//!
//! Because the sweep is a reconcile rather than a hook on any particular
//! spell, every present and future way of losing flight is already wired:
//! the fall is a consequence of the actor's state, not of the code path
//! that changed it. That is the same reason `reconcile_footprints` is a
//! sweep — a growth effect and a shrink effect must not each have to
//! remember to fix the map.
//!
//! # Why one altitude and not a per-source table
//!
//! [`FLIGHT_ALTITUDE_FT`] is a single constant rather than a row per
//! flight source. RAW does not assign a height to any of them — a flying
//! speed is permission to choose one, and the creature chooses every
//! turn. Reading three different numbers off three spells that all grant
//! "a flying speed of 60 feet" would be inventing precision the rules
//! never had, and the invented numbers would then be load-bearing on the
//! damage. One documented altitude for "airborne under your own magic"
//! is the honest translation. If a future source ever genuinely differs
//! — a Levitate that pins RAW's exact 20 feet, a wind zone that lifts
//! whoever enters it — this constant becomes a cohort table beside
//! `CONDITION_SPEED_BONUSES` and the sweep reads the max instead.
//!
//! # What deliberately does not fall
//!
//!   - **`Lifted`** (Levitate / Telekinesis). It is someone else's
//!     concentration holding the target up, and RAW's Levitate ends with
//!     the target floating *gently* to the ground. It is absent from
//!     `MAGICAL_FLIGHT_CONDITIONS` for the neighbouring reason (Earthbind
//!     has no flying speed to strip) and absent here for this one.
//!   - **A controlled descent.** Earthbind's RAW is "an airborne creature
//!     affected by this spell descends at 60 feet per round until it
//!     reaches the ground" — a landing, not a fall. It routes through the
//!     `LandSafely` side effect, which zeroes the altitude and strips the
//!     flight in one atomic apply so the sweep never sees an actor
//!     holding flight at altitude zero and hoists them back up.
//!   - **A feathered creature.** See `Condition::Feathered` and
//!     `EncounterInstance::try_feather_fall`.

use crate::engine::dice::Dice;

/// Feet of fall bought by each damage die. SRD: "1d6 bludgeoning damage
/// for every 10 feet it fell".
pub const FEET_PER_FALL_DIE: u32 = 10;

/// Faces on the fall die. SRD: d6, bludgeoning.
pub const FALL_DIE_FACES: u32 = 6;

/// SRD's cap: "to a maximum of 20d6", i.e. everything past 200 feet is
/// the same fall. Nothing in this engine climbs that high — the cap is
/// here because it is the rule, and because a future effect that hurls a
/// creature skyward would otherwise scale without bound.
pub const MAX_FALL_DICE: u32 = 20;

/// The one height the engine puts a self-powered flier at, in feet.
///
/// Thirty feet is half a round's climb on RAW's 60 ft flying speed, and
/// the height at which a Medium creature on the floor can no longer
/// reach the flier with a 5 ft weapon — high enough that being up there
/// meant something, low enough that the 3d6 owed on the way down (10.5
/// average) is a real cost rather than a death sentence for the d6-hit-die
/// chassis that casts Fly in the first place.
///
/// See the module docs for why this is a constant and not a table.
pub const FLIGHT_ALTITUDE_FT: u32 = 30;

/// The damage pool owed for a fall of `distance_ft` feet.
///
/// Integer division is the rule, not a rounding convenience: SRD pays a
/// die "for every 10 feet", so a 25-foot drop is 2d6 and the leftover 5
/// feet buy nothing. A fall shorter than [`FEET_PER_FALL_DIE`] yields a
/// zero-count pool, which every roller in the engine evaluates to 0 —
/// so callers get "no damage" without a special case.
pub fn fall_damage_dice(distance_ft: u32) -> Dice {
    Dice::new(
        (distance_ft / FEET_PER_FALL_DIE).min(MAX_FALL_DICE),
        FALL_DIE_FACES,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fall_pays_one_die_per_ten_feet_and_keeps_the_change() {
        // The three interesting rungs: under a die, exactly a die, and a
        // remainder that buys nothing.
        assert_eq!(fall_damage_dice(9).count, 0, "a 9 ft drop is free");
        assert_eq!(fall_damage_dice(10).count, 1);
        assert_eq!(
            fall_damage_dice(25).count,
            2,
            "25 ft is two dice and a wasted 5 ft, not two and a half"
        );
        assert_eq!(
            fall_damage_dice(FLIGHT_ALTITUDE_FT).count,
            3,
            "the engine's cruising altitude is worth 3d6"
        );
    }

    #[test]
    fn the_srd_cap_holds_however_far_the_drop() {
        // 200 ft is exactly the cap; past it every fall is the same fall,
        // including one that would overflow a naive multiply.
        assert_eq!(fall_damage_dice(200).count, MAX_FALL_DICE);
        assert_eq!(fall_damage_dice(1_000).count, MAX_FALL_DICE);
        assert_eq!(fall_damage_dice(u32::MAX).count, MAX_FALL_DICE);
    }

    #[test]
    fn every_fall_pool_rolls_d6() {
        for ft in [0, 10, 55, 200, 5_000] {
            assert_eq!(
                fall_damage_dice(ft).faces,
                FALL_DIE_FACES,
                "fall damage is always d6, bludgeoning"
            );
        }
    }
}
