//! Persistent magical areas — the "zone" layer.
//!
//! 5e is full of spells whose effect is a *place* rather than a target:
//! Web, Fog Cloud, Grease, Cloud of Daggers, Moonbeam, Sleet Storm. What
//! they all say is some variation of one sentence — "when a creature
//! enters the area for the first time on a turn or starts its turn
//! there, it …" — plus, for some, a standing clause on the ground
//! itself ("the area is difficult terrain", "the area is heavily
//! obscured").
//!
//! Before this module the engine had no way to say that. Area spells
//! resolved once, against whoever happened to be standing in the burst
//! at the instant of the cast, and then stopped existing; Fog Cloud's
//! own docstring named the gap outright ("the engine has no 'is this
//! tile fog-covered' terrain layer to query for moves yet"). The
//! consequences were visible at the table: a creature could walk into a
//! web and be unaffected, and a creature could walk *out* of a fog
//! cloud and stay blind until the caster's concentration lapsed.
//!
//! A zone is deliberately not a terrain type. Terrain is the map — it
//! is generated once, it has no owner, and it does not end. A zone is
//! owned (by the caster, whose save DC it carries and whose
//! concentration holds it up), it expires, and two of them can sit on
//! the same tile. Keeping them as a separate layer that *composes* with
//! terrain is what lets a web laid over rubble be difficult terrain for
//! one reason rather than two, and lets the whole layer be torn down by
//! id when the caster goes down.
//!
//! ## What a zone can do
//!
//! Three clauses, each independently optional, which between them cover
//! every stationary-area spell the engine has reason to model:
//!
//!   - **`obscures`** — heavy obscurement. Blocks sight into, out of,
//!     and through the area (read by `EncounterInstance::viewer_can_see`).
//!   - **`difficult`** — the movement surcharge, composed with the
//!     terrain layer's own by `max` rather than by multiplication, so a
//!     web over rubble costs 2× and not 4×.
//!   - **`contact`** — the "enters for the first time on a turn or
//!     starts its turn there" clause: an optional save, optional
//!     damage, optional condition.
//!
//! ## What a zone deliberately isn't
//!
//! **It has no shape but a square.** `radius` is a Chebyshev radius
//! around `origin`, the same measure every burst in the engine already
//! uses. Walls and lines would each need their own geometry and their
//! own answer for "which tiles does a 4-tile line through a doorway
//! cover"; a burst reuses `footprint_chebyshev` and is the shape all
//! four spells this lands with actually have.
//!
//! **It doesn't move.** RAW lets a Moonbeam be walked around the board
//! for an action. Nothing here forbids adding that later — `origin` is
//! a plain field — but no zone the engine installs today moves, so
//! there is no mover.
//!
//! **It is friend-or-foe blind.** A web catches the wizard who cast it.
//! That is RAW, it is what makes placement a decision, and the AI is
//! taught to respect it (`SimpleAi` prices a harmful zone into its
//! pathing) rather than the zone being taught to respect the AI.

use crate::conditions::{Condition, ConditionTimer};
use crate::engine::dice::Dice;
use crate::engine::types::{AbilityScoreType, Coordinate, DamageType};

/// The saving throw a zone's contact clause opens with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZoneSave {
    pub ability: AbilityScoreType,
    /// Snapshotted from the caster's spell save DC at install time
    /// rather than read live off the owner. A zone outlives the state
    /// that made it — the caster can be blinded, shrunk, or killed
    /// while their web keeps holding people — and RAW fixes a spell's
    /// DC at the moment it is cast.
    pub dc: i32,
    /// True for the "half damage on a successful save" spells (Moonbeam,
    /// Sleet Storm's cousins). False for the ones whose save negates
    /// outright (Web, Grease).
    pub half_on_success: bool,
}

/// What happens to a creature that enters the zone for the first time on
/// a turn, or starts its turn inside it.
///
/// All three fields are optional and they compose: a save alone (Web),
/// damage alone (Cloud of Daggers), or a save that gates a condition
/// (Grease). A `ZoneContact` with everything `None` is inert and would
/// be better expressed as `contact: None`; nothing forbids it, and
/// `is_harmful` reports such a zone as harmless, which is the honest
/// answer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZoneContact {
    pub save: Option<ZoneSave>,
    pub damage: Option<(Dice, DamageType)>,
    pub condition: Option<(Condition, ConditionTimer)>,
    /// 5e Sleet Storm: "any creature that enters the area or starts its
    /// turn there must make a Constitution saving throw or lose
    /// concentration." A separate roll from `save`, at the same DC,
    /// because the two clauses ask different abilities of the same
    /// creature — a wizard keeps its feet on a Dexterity save and keeps
    /// its spell on a Constitution one, and passing either says nothing
    /// about the other.
    ///
    /// Only ever charged to a creature that has concentration to lose.
    pub breaks_concentration: bool,
}

impl ZoneContact {
    /// A save-or-suffer clause with no damage: Web's Restrained, Grease's
    /// Prone.
    pub const fn save_or(
        ability: AbilityScoreType,
        dc: i32,
        condition: Condition,
        timer: ConditionTimer,
    ) -> Self {
        Self {
            save: Some(ZoneSave {
                ability,
                dc,
                half_on_success: false,
            }),
            damage: None,
            condition: Some((condition, timer)),
            breaks_concentration: false,
        }
    }

    /// Chainable: this clause also forces a concentration check at the
    /// same DC. Sleet Storm's second sentence.
    pub const fn also_breaking_concentration(mut self) -> Self {
        self.breaks_concentration = true;
        self
    }

    /// The "save for half" shape: Moonbeam's searing light, and every
    /// damaging area whose save softens the blow rather than dodging it.
    pub const fn save_for_half(
        ability: AbilityScoreType,
        dc: i32,
        dice: Dice,
        damage_type: DamageType,
    ) -> Self {
        Self {
            save: Some(ZoneSave {
                ability,
                dc,
                half_on_success: true,
            }),
            damage: Some((dice, damage_type)),
            condition: None,
            breaks_concentration: false,
        }
    }

    /// Unavoidable damage on contact: Cloud of Daggers, which offers no
    /// save at all.
    pub const fn damage(dice: Dice, damage_type: DamageType) -> Self {
        Self {
            save: None,
            damage: Some((dice, damage_type)),
            condition: None,
            breaks_concentration: false,
        }
    }

    /// True if this clause can cost a creature something. Read by the AI
    /// so it can route around a web and stand in a fog cloud.
    pub fn is_harmful(&self) -> bool {
        self.damage.is_some() || self.condition.is_some() || self.breaks_concentration
    }
}

/// The standing clauses a zone lays on the ground under it, plus the
/// contact clause it fires at creatures in it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZoneEffect {
    /// 5e **heavily obscured**: "a creature effectively suffers from the
    /// blinded condition when trying to see something in that area."
    /// Read symmetrically — a fog cloud blinds you to what is inside it
    /// whether you are standing in it, on the far side of it, or
    /// looking out of it.
    pub obscures: bool,
    /// 5e **difficult terrain**: each tile costs double.
    pub difficult: bool,
    pub contact: Option<ZoneContact>,
}

impl ZoneEffect {
    /// A zone whose only clause is the obscurement (Fog Cloud).
    pub const OBSCURING: Self = Self {
        obscures: true,
        difficult: false,
        contact: None,
    };

    /// A zone whose only clause is the bad ground (Entangle, whose
    /// grab is a one-time save at cast time rather than a standing
    /// property of the square).
    pub const ROUGH: Self = Self {
        obscures: false,
        difficult: true,
        contact: None,
    };

    /// A zone that is difficult terrain and fires `contact` (Web,
    /// Grease).
    pub const fn clinging(contact: ZoneContact) -> Self {
        Self {
            obscures: false,
            difficult: true,
            contact: Some(contact),
        }
    }

    /// A zone that both blinds and bites: the poison and vapor clouds,
    /// whose RAW text carries "its area is heavily obscured" alongside
    /// the save (Cloudkill, Stinking Cloud).
    pub const fn choking(contact: ZoneContact) -> Self {
        Self {
            obscures: true,
            difficult: false,
            contact: Some(contact),
        }
    }

    /// A zone that fires `contact` and nothing else — the ground under
    /// it is unchanged (Cloud of Daggers).
    pub const fn hazard(contact: ZoneContact) -> Self {
        Self {
            obscures: false,
            difficult: false,
            contact: Some(contact),
        }
    }

    /// True if standing in this zone can cost a creature something.
    /// Obscurement doesn't count: it is as much a hiding place as a
    /// handicap, and the AI treats it as free ground.
    pub fn is_harmful(&self) -> bool {
        self.contact.is_some_and(|c| c.is_harmful())
    }
}

/// One persistent magical area on the board.
#[derive(Debug, Clone, PartialEq)]
pub struct Zone {
    /// Unique per encounter, handed out by
    /// `EncounterInstance::install_zone`. Identity matters because the
    /// "first time on a turn" ledger is keyed by `(zone id, actor id)`:
    /// two overlapping webs each get their own save.
    pub id: usize,
    pub name: &'static str,
    /// The caster. Used for concentration teardown and for the log line;
    /// deliberately *not* used to spare the owner from the zone's own
    /// contact clause.
    pub owner_id: usize,
    pub origin: Coordinate,
    /// Chebyshev radius in tiles. `0` is a single tile.
    pub radius: isize,
    pub effect: ZoneEffect,
    /// Rounds left, decremented at each round end. A zone installed with
    /// `0` is already over and will be swept on the next round end
    /// rather than never — install with at least 1.
    pub rounds_remaining: u32,
    /// True if the owner's concentration holds it up, in which case
    /// `drop_concentration` tears it down early.
    pub concentration: bool,
}

impl Zone {
    /// True if `coord` is inside the zone. Single-tile question; callers
    /// with a footprint use `EncounterInstance::actor_in_zone`, which
    /// asks the same question of the whole body.
    pub fn covers(&self, coord: Coordinate) -> bool {
        self.origin.chebyshev_to(coord) <= self.radius
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zone_at(origin: Coordinate, radius: isize) -> Zone {
        Zone {
            id: 1,
            name: "test",
            owner_id: 0,
            origin,
            radius,
            effect: ZoneEffect::OBSCURING,
            rounds_remaining: 10,
            concentration: false,
        }
    }

    #[test]
    fn a_zero_radius_zone_is_one_tile() {
        let z = zone_at(Coordinate::new(5, 5), 0);
        assert!(z.covers(Coordinate::new(5, 5)));
        assert!(!z.covers(Coordinate::new(5, 6)));
    }

    /// Chebyshev, not Euclidean: the corner of the square is in, which
    /// is how every other burst in the engine measures.
    #[test]
    fn the_radius_is_a_square_not_a_circle() {
        let z = zone_at(Coordinate::new(5, 5), 2);
        assert!(z.covers(Coordinate::new(7, 7)));
        assert!(!z.covers(Coordinate::new(8, 7)));
    }

    #[test]
    fn obscurement_alone_is_not_harmful() {
        assert!(!ZoneEffect::OBSCURING.is_harmful());
    }

    #[test]
    fn a_contact_clause_that_only_saves_is_still_harmful() {
        let effect = ZoneEffect::clinging(ZoneContact::save_or(
            AbilityScoreType::Dexterity,
            13,
            Condition::Restrained,
            ConditionTimer::Rounds(10),
        ));
        assert!(effect.is_harmful());
        assert!(effect.difficult);
    }

    #[test]
    fn a_damage_zone_leaves_the_ground_alone() {
        let effect = ZoneEffect::hazard(ZoneContact::damage(
            Dice::new(4, 4),
            DamageType::Slashing,
        ));
        assert!(effect.is_harmful());
        assert!(!effect.difficult);
        assert!(!effect.obscures);
    }
}
