//! Persistent magical areas — the "zone" layer.
//!
//! 5e is full of spells whose effect is a *place* rather than a target:
//! Web, Fog Cloud, Grease, Darkness, Silence, Spike Growth, Moonbeam,
//! Cloudkill, Sickening Radiance, Hunger of Hadar, and a dozen more.
//! What they all say is some variation of one sentence — "when a
//! creature enters the area for the first time on a turn or starts its
//! turn there, it …" — plus, for many, a standing clause on the ground
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
//! Five clauses, each independently optional, which between them cover
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
//!   - **`per_step_damage`** — the one trigger 5e bills by the tile
//!     rather than by the turn: Spike Growth's "2d4 piercing for every
//!     5 feet it travels".
//!   - **`suppresses_magic`** — Antimagic Field's "spells … are
//!     suppressed in the sphere and can't protrude into it", read at
//!     the casting gate rather than at a contact trigger.
//!
//! ## Where a zone is
//!
//! Most of them are wherever they were put, forever — a web is spun on
//! a patch of floor and that is the patch of floor it holds. But 5e has
//! a whole cohort whose defining sentence is about *going somewhere*,
//! and `motion` is the axis that carries them:
//!
//!   - **`ZoneMotion::Fixed`** — the default, and the right answer for
//!     every ground-laid area.
//!   - **`ZoneMotion::DriftsFromOwner`** — Cloudkill's "the cloud moves
//!     10 feet away from you at the start of each of your turns", and
//!     Incendiary Cloud's identical clause. Nobody spends anything; the
//!     cloud walks itself, and walking itself off the far end of the
//!     board is how those two spells stop being the caster's problem.
//!   - **`ZoneMotion::Directed`** — Moonbeam's "you can move the beam up
//!     to 60 feet" and the bonus-action siblings (Dawn, Flaming
//!     Sphere). The owner pays for it, and pays on their own turn.
//!
//! Both movers land on `EncounterInstance::move_zone`, which is also
//! where the one rule a moving area needs that a still one doesn't
//! lives: the contact clause fires at whoever the area has *newly*
//! covered, and at nobody else. Moonbeam says so outright ("when you
//! move the beam into a creature's space"), and the alternative — a
//! blanket re-charge of everyone underneath — would bill the creature
//! the beam was already on twice for standing still.
//!
//! ## What a zone deliberately isn't
//!
//! **It has no shape but a square.** `radius` is a Chebyshev radius
//! around `origin`, the same measure every burst in the engine already
//! uses. Lines would need their own geometry and their own answer for
//! "which tiles does a 4-tile line through a doorway cover"; a burst
//! reuses `footprint_chebyshev`, and every spell on the layer is a
//! sphere, a cube, or a square.
//!
//! The hazard walls — Wall of Fire, Blade Barrier, Wall of Thorns — do
//! live here, as squares. What they needed from the layer was
//! *persistence*, not geometry: the clause that makes each of them a
//! wall rather than a burst is "when a creature enters it for the first
//! time on a turn or ends its turn there", and that sentence is what a
//! zone is. A wide square is a coarse wall and a perfectly good one.
//!
//! The walls that are really about geometry — the ones whose whole
//! effect is that you cannot get past them — are not zones at all.
//! Wall of Stone and Wall of Force write terrain, through
//! `crate::engine::conjured_terrain`, because what they need is for the
//! pathfinder and the line-of-sight walk to see them, and both of those
//! read the map rather than this layer.
//!
//! **It is friend-or-foe blind.** A web catches the wizard who cast it.
//! That is RAW, it is what makes placement a decision, and the AI is
//! taught to respect it (`SimpleAi` prices a harmful zone into its
//! pathing, and its burst-placement picker refuses a spot that catches
//! its own side) rather than the zone being taught to respect the AI.
//!
//! **It carries one damage roll.** Hunger of Hadar wants two, on
//! different terms — automatic cold and saved-against acid — and gets
//! them by going down as two coincident zones rather than by widening
//! the contact shape for the one spell in 5e that asks.

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

    /// A condition the area simply imposes, with no save and no
    /// damage: Silence's hush, which nobody rolls against.
    ///
    /// Paired with `ConditionTimer::UntilStartOfNextTurn`, this is how
    /// the layer expresses a condition that lasts *while you are in
    /// here*. The zone re-installs it at the top of every turn spent
    /// inside, and it lapses on its own for a creature that has walked
    /// out — which is one tick later than RAW and infinitely closer
    /// than a condition stamped on at cast time and never removed.
    pub const fn afflicts(condition: Condition, timer: ConditionTimer) -> Self {
        Self {
            save: None,
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

    /// A save that negates both a hit and a hold: Evard's Black
    /// Tentacles, whose Dexterity save is the difference between
    /// nothing at all and 3d6 plus being pinned.
    pub const fn save_or_suffer(
        ability: AbilityScoreType,
        dc: i32,
        dice: Dice,
        damage_type: DamageType,
        condition: Condition,
        timer: ConditionTimer,
    ) -> Self {
        Self {
            save: Some(ZoneSave {
                ability,
                dc,
                half_on_success: false,
            }),
            damage: Some((dice, damage_type)),
            condition: Some((condition, timer)),
            breaks_concentration: false,
        }
    }

    /// A save that negates the damage outright rather than halving it:
    /// Hunger of Hadar's acid, whose Dexterity save is all-or-nothing.
    pub const fn save_or_take(
        ability: AbilityScoreType,
        dc: i32,
        dice: Dice,
        damage_type: DamageType,
    ) -> Self {
        Self {
            save: Some(ZoneSave {
                ability,
                dc,
                half_on_success: false,
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
    /// 5e Spike Growth: "when a creature moves into or within the area,
    /// it takes 2d4 piercing damage for every 5 feet it travels."
    ///
    /// A third trigger, and the only one in 5e that is charged by the
    /// *tile* rather than by the turn — which is why it can't be folded
    /// into `contact`. Contact fires once however far you walk;
    /// this fires once per step, and the difference is the whole spell:
    /// the thorns punish crossing the patch, not being on it.
    pub per_step_damage: Option<(Dice, DamageType)>,
    /// 5e **Antimagic Field**: "spells and other magical effects …
    /// are suppressed in the sphere and can't protrude into it."
    ///
    /// The fourth axis, and the only one that is about what a creature
    /// may *do* rather than about what the ground does to it. A
    /// suppressing area has no save, no damage and no condition —
    /// nothing lands on anyone standing in it — so it is invisible to
    /// `is_harmful` and the AI walks through it freely, which is right:
    /// for a creature that does not cast, the field is empty air.
    ///
    /// What it does is close the casting gate at
    /// `Action::validate_input`, in both directions. A caster inside
    /// cannot cast at all; a caster outside cannot reach a target
    /// inside. Both halves read one predicate,
    /// `EncounterInstance::magic_suppressed_between`, so a spell aimed
    /// at a point and a spell aimed at a creature are answered by the
    /// same sentence.
    pub suppresses_magic: bool,
}

impl ZoneEffect {
    /// A zone whose only clause is the obscurement (Fog Cloud).
    pub const OBSCURING: Self = Self {
        obscures: true,
        difficult: false,
        contact: None,
        per_step_damage: None,
        suppresses_magic: false,
    };

    /// A zone whose only clause is the bad ground (Entangle, whose
    /// grab is a one-time save at cast time rather than a standing
    /// property of the square).
    pub const ROUGH: Self = Self {
        obscures: false,
        difficult: true,
        contact: None,
        per_step_damage: None,
        suppresses_magic: false,
    };

    /// A zone whose only clause is that magic does not work inside it
    /// (Antimagic Field). Nothing about the ground changes and nothing
    /// is rolled — see `suppresses_magic`.
    pub const NULLIFYING: Self = Self {
        obscures: false,
        difficult: false,
        contact: None,
        per_step_damage: None,
        suppresses_magic: true,
    };

    /// A zone that is difficult terrain and fires `contact` (Web,
    /// Grease).
    pub const fn clinging(contact: ZoneContact) -> Self {
        Self {
            obscures: false,
            difficult: true,
            contact: Some(contact),
            per_step_damage: None,
            suppresses_magic: false,
        }
    }

    /// Ground that charges by the tile: Spike Growth's thorns, which
    /// are difficult terrain *and* bill 2d4 for every 5 ft crossed.
    pub const fn thorny(dice: Dice, damage_type: DamageType) -> Self {
        Self {
            obscures: false,
            difficult: true,
            contact: None,
            per_step_damage: Some((dice, damage_type)),
            suppresses_magic: false,
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
            per_step_damage: None,
            suppresses_magic: false,
        }
    }

    /// A zone that fires `contact` and nothing else — the ground under
    /// it is unchanged (Cloud of Daggers).
    pub const fn hazard(contact: ZoneContact) -> Self {
        Self {
            obscures: false,
            difficult: false,
            contact: Some(contact),
            per_step_damage: None,
            suppresses_magic: false,
        }
    }

    /// True if standing in this zone can cost a creature something.
    /// Obscurement doesn't count: it is as much a hiding place as a
    /// handicap, and the AI treats it as free ground.
    pub fn is_harmful(&self) -> bool {
        self.contact.is_some_and(|c| c.is_harmful()) || self.per_step_damage.is_some()
    }
}

/// How a persistent area moves once it is on the board.
///
/// The variants are distinguished by *who pays*, which is the only
/// question the engine has to answer differently for each of them:
/// `DriftsFromOwner` is charged to nobody and so is run by the engine
/// at the top of the owner's turn, while `Directed` is charged to the
/// owner's action economy and so is run by the spell that owns the
/// zone, on the turn the owner decides to spend it.
///
/// `tiles` is in tiles on the engine's 2.5-ft grid throughout: 10 ft is
/// 4, 30 ft is 12, 60 ft is 24.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoneMotion {
    /// The area holds the ground it was laid on. Web, Grease, Fog
    /// Cloud, Spike Growth — everything whose placement is a decision
    /// made once.
    Fixed,
    /// 5e Cloudkill / Incendiary Cloud: "the cloud moves 10 feet away
    /// from you at the start of each of your turns."
    ///
    /// The direction is the unit step pointing from the owner to the
    /// area's current origin, so the cloud keeps travelling along the
    /// line it was cast down. A cloud centred exactly on its owner has
    /// no "away" to move along and stays put for that turn — which is
    /// self-correcting, since the owner only has to take a step for the
    /// drift to pick a direction again.
    DriftsFromOwner { tiles: isize },
    /// 5e Moonbeam / Dawn / Flaming Sphere: the owner may reposition the
    /// area on their own turn, up to `tiles` from where it stands.
    ///
    /// The engine does not move these — the spell does, by being cast
    /// again while its zone is already up. That is what makes the
    /// action economy come out right without a second action per spell:
    /// the recast is where a cost lives, and each of the three spells
    /// charges a different one (Moonbeam an action, Dawn and Flaming
    /// Sphere a bonus action) with no spell slot behind it.
    Directed { tiles: isize },
    /// 5e Antimagic Field: "a … sphere of antimagic surrounds you …
    /// The sphere moves with you."
    ///
    /// The one motion with no distance on it, because there is no
    /// distance in the sentence: the area is not travelling anywhere,
    /// it is *attached*. Re-centred on the owner at the top of their
    /// turn, in the same pass that drifts the clouds — so between
    /// turns it lags a step behind a caster who has walked, which is
    /// exactly the granularity `DriftsFromOwner` already accepts and
    /// for the same reason: the alternative is a zone update on every
    /// tile of every move.
    ///
    /// An owner who has left the board takes the area with them, the
    /// same as one whose cloud drifts off the edge.
    FollowsOwner,
}

impl ZoneMotion {
    /// How far the owner may shift this area on their own turn, or
    /// `None` for an area they don't steer. Read by the repositioning
    /// half of a `Directed` spell's validation, so the range clause
    /// lives on the zone rather than being restated by each spell.
    pub fn directed_range(self) -> Option<isize> {
        match self {
            ZoneMotion::Directed { tiles } => Some(tiles),
            _ => None,
        }
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
    /// Whether the area travels, and on whose account — see
    /// `ZoneMotion`. `Fixed` for all but the drifting clouds and the
    /// steered beams.
    pub motion: ZoneMotion,
}

impl Zone {
    /// True if `coord` is inside the zone. Single-tile question; callers
    /// with a footprint use `EncounterInstance::actor_in_zone`, which
    /// asks the same question of the whole body.
    pub fn covers(&self, coord: Coordinate) -> bool {
        self.origin.chebyshev_to(coord) <= self.radius
    }

    /// Where a `DriftsFromOwner` area would end up this turn, given
    /// where its owner is standing. `None` for an area that doesn't
    /// drift, and for the degenerate case of a cloud centred on the
    /// creature it is supposed to be moving away from — there is no
    /// direction in that sentence, and inventing one would send the
    /// cloud somewhere the table can't predict.
    ///
    /// The step is taken per-axis by sign, so a cloud cast diagonally
    /// keeps travelling diagonally and one cast straight down a
    /// corridor keeps travelling down it.
    pub fn drift_destination(&self, owner_at: Coordinate) -> Option<Coordinate> {
        let ZoneMotion::DriftsFromOwner { tiles } = self.motion else {
            return None;
        };
        let away = self.origin - owner_at;
        if away.x == 0 && away.y == 0 {
            return None;
        }
        Some(self.origin + Coordinate::new(away.x.signum() * tiles, away.y.signum() * tiles))
    }

    /// Where this area belongs at the top of its owner's turn, given
    /// where the owner is standing — or `None` if it belongs exactly
    /// where it is.
    ///
    /// The engine-run half of `ZoneMotion`: the two variants nobody
    /// spends anything on. A drifting cloud takes its step; an attached
    /// sphere snaps back onto its owner. `Fixed` and `Directed` answer
    /// `None`, the former because it never moves and the latter because
    /// its owner moves it by hand and pays for it.
    pub fn turn_start_destination(&self, owner_at: Coordinate) -> Option<Coordinate> {
        match self.motion {
            ZoneMotion::DriftsFromOwner { .. } => self.drift_destination(owner_at),
            ZoneMotion::FollowsOwner => (self.origin != owner_at).then_some(owner_at),
            ZoneMotion::Fixed | ZoneMotion::Directed { .. } => None,
        }
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
            motion: ZoneMotion::Fixed,
        }
    }

    fn drifting_zone_at(origin: Coordinate, owner_at: Coordinate) -> (Zone, Coordinate) {
        let mut z = zone_at(origin, 2);
        z.motion = ZoneMotion::DriftsFromOwner { tiles: 4 };
        (z, owner_at)
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

    /// A still area is still: nothing about the default motion asks the
    /// engine to move anything.
    #[test]
    fn a_fixed_zone_has_nowhere_to_drift() {
        let z = zone_at(Coordinate::new(5, 5), 2);
        assert_eq!(z.motion, ZoneMotion::Fixed);
        assert!(z.drift_destination(Coordinate::new(1, 1)).is_none());
    }

    /// "Moves 10 feet away from you" — along the line from the owner
    /// through the cloud, which for a cloud cast straight down a
    /// corridor keeps it in the corridor.
    #[test]
    fn a_drifting_cloud_travels_away_along_the_axis_it_was_cast_down() {
        let (z, owner) = drifting_zone_at(Coordinate::new(10, 5), Coordinate::new(2, 5));
        assert_eq!(
            z.drift_destination(owner),
            Some(Coordinate::new(14, 5)),
            "four tiles further along +x, and not a tile off the row"
        );
    }

    /// The step is taken per axis by sign, so a diagonal cast keeps
    /// travelling diagonally rather than snapping onto an axis.
    #[test]
    fn a_diagonal_drift_stays_diagonal() {
        let (z, owner) = drifting_zone_at(Coordinate::new(8, 8), Coordinate::new(4, 2));
        assert_eq!(z.drift_destination(owner), Some(Coordinate::new(12, 12)));
    }

    /// A cloud centred on its owner has no "away" in it. Standing still
    /// is the honest answer; picking a direction would make the cloud's
    /// path depend on something the table can't see.
    #[test]
    fn a_cloud_sitting_on_its_owner_has_no_direction_to_take() {
        let (z, _) = drifting_zone_at(Coordinate::new(6, 6), Coordinate::new(6, 6));
        assert!(z.drift_destination(Coordinate::new(6, 6)).is_none());
    }

    /// The steered cohort reports its own leash so the spells that own
    /// them don't each restate the range.
    #[test]
    fn only_a_directed_zone_reports_a_reposition_range() {
        assert_eq!(
            ZoneMotion::Directed { tiles: 24 }.directed_range(),
            Some(24)
        );
        assert_eq!(ZoneMotion::Fixed.directed_range(), None);
        assert_eq!(
            ZoneMotion::DriftsFromOwner { tiles: 4 }.directed_range(),
            None,
            "a cloud that walks itself is not a cloud the owner can aim"
        );
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

    /// The two save polarities are not interchangeable, and a spell
    /// that picks the wrong one is either twice as strong or half as
    /// strong as it reads. Pinned because the difference is one bool.
    #[test]
    fn a_save_either_halves_the_damage_or_negates_it() {
        let half = ZoneContact::save_for_half(
            AbilityScoreType::Constitution,
            15,
            Dice::new(5, 8),
            DamageType::Poison,
        );
        assert!(half.save.is_some_and(|s| s.half_on_success));
        let negates = ZoneContact::save_or_take(
            AbilityScoreType::Dexterity,
            15,
            Dice::new(2, 6),
            DamageType::Acid,
        );
        assert!(negates.save.is_some_and(|s| !s.half_on_success));
    }

    /// Black Tentacles' clause: one save standing between a creature
    /// and both a hit and a hold.
    #[test]
    fn one_save_can_gate_both_damage_and_a_condition() {
        let contact = ZoneContact::save_or_suffer(
            AbilityScoreType::Dexterity,
            15,
            Dice::new(3, 6),
            DamageType::Bludgeoning,
            Condition::Restrained,
            ConditionTimer::Rounds(10),
        );
        assert!(contact.damage.is_some());
        assert!(contact.condition.is_some());
        assert!(contact.save.is_some_and(|s| !s.half_on_success));
        assert!(contact.is_harmful());
    }

    /// A clause whose only effect is to break concentration still
    /// counts as harmful — the AI has to route around sleet even
    /// though nothing in it deals damage or holds anyone.
    #[test]
    fn breaking_concentration_alone_makes_a_clause_harmful() {
        let bare = ZoneContact {
            save: Some(ZoneSave {
                ability: AbilityScoreType::Dexterity,
                dc: 15,
                half_on_success: false,
            }),
            damage: None,
            condition: None,
            breaks_concentration: true,
        };
        assert!(bare.is_harmful());
        // And the chainable form composes onto a real clause without
        // disturbing it.
        let sleet = ZoneContact::save_or(
            AbilityScoreType::Dexterity,
            15,
            Condition::Prone,
            ConditionTimer::Permanent,
        )
        .also_breaking_concentration();
        assert!(sleet.breaks_concentration);
        assert_eq!(sleet.condition, Some((Condition::Prone, ConditionTimer::Permanent)));
    }

    /// Thorny ground is harmful on the strength of the per-step clause
    /// alone, with no contact clause at all — which is the only way the
    /// AI's hazard check sees Spike Growth.
    #[test]
    fn per_step_damage_alone_makes_a_zone_harmful() {
        let effect = ZoneEffect::thorny(Dice::new(2, 4), DamageType::Piercing);
        assert!(effect.contact.is_none());
        assert!(effect.difficult);
        assert!(effect.is_harmful());
        // Bad ground with nothing else on it is not.
        let rough = ZoneEffect::ROUGH;
        assert!(!rough.is_harmful());
        assert!(rough.difficult);
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
