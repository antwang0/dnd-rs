//! **Footing** — the tile that can knock you over.
//!
//! SRD 5.2, *Environmental Effects*, **Slippery Ice**:
//!
//! > Slippery ice is Difficult Terrain. A creature that moves onto
//! > slippery ice for the first time on a turn or starts its turn there
//! > must succeed on a DC 10 Dexterity saving throw or have the Prone
//! > condition.
//!
//! The movement half of that is [`TerrainType::movement_cost`] and needs
//! nothing from this module. The save is what does, and it is the first
//! time the **terrain layer** has had a trigger rather than only a
//! price.
//!
//! ## Why this is not a zone
//!
//! It reads exactly like one. The trigger is word-for-word the trigger
//! [`crate::engine::zones`] was built for — *"enters the area for the
//! first time on a turn or starts its turn there"* — and Sleet Storm,
//! which is this rule cast as a spell, is a `ZoneEffect::clinging` with
//! a save-or-Prone contact and could not be anything else.
//!
//! The difference is ownership, and it is the same line the
//! zone/`conjured_terrain` split already draws. A zone is somebody's:
//! it has an `owner_id` whose spell save DC it rolls against, a
//! `rounds_remaining`, and a caster whose concentration ends it. Ice on
//! the floor of a cavern is nobody's. It has no DC of its own to lend —
//! RAW hands it a flat 10 — nothing to expire, and no grip to break.
//! Installing a permanent, ownerless zone over every ice tile on the map
//! would put a synthetic caster on the board and pay the zone layer's
//! per-tile overlap cost for a fact the terrain array already holds.
//!
//! So the rule lives beside the tile, and what it borrows from the zone
//! layer is the *shape* rather than the machinery: one ledger, cleared
//! at the same moment, read at the same two sites.
//!
//! ## The two triggers are one call
//!
//! `EncounterInstance::test_footing` is asked at both, exactly as
//! `touch_zones` is, and for the same reason — RAW writes them as one
//! sentence with two clocks. Both go through
//! `EncounterInstance::touch_ground`, which is the single "this creature
//! is now standing here" hook every arrival in the engine already routed
//! through in its zone half: a walk, a step of a walk, a teleport, a
//! shove, a pull, a banishment that lapsed, a swallowed creature cut
//! free.
//!
//! The once-per-turn ledger is what makes a six-tile skate across a pond
//! one save rather than six, and it is cleared for the **whole board**
//! at the top of every turn rather than for the creature whose turn is
//! opening — because RAW scopes *"the first time on a turn"* to the
//! turn, not to the holder's own turn. A creature shoved onto the ice
//! during somebody else's turn has moved onto it for the first time on
//! that turn, and it saves.
//!
//! ## Who is asked
//!
//! The creature whose feet are on the tile, which is not always the
//! creature being moved:
//!
//!   - **A rider is not asked; its mount is.** Slipping is a fact about
//!     what is touching the ice, and a knight on a horse is touching the
//!     horse. This is the same redirect `is_immersed` makes through
//!     `movement_body`, and for the same reason — the mount is the body
//!     on the board.
//!   - **Nothing in the air is asked.** `is_grounded` is the gate, so a
//!     flier crossing the pond, a creature `Lifted` by Telekinesis and a
//!     burrower under the shore all cross it without rolling. A
//!     burrower is covered twice over, since `is_airborne` already
//!     refuses anything underground.
//!   - **Nobody already Prone is asked.** You cannot fall over from the
//!     floor, and rolling for it would print a save in the log with
//!     nothing behind it either way.
//!   - **Nobody immune to Prone is asked**, through the same
//!     `actor_immune_to_condition` gate every other condition install
//!     goes through — which is what makes an ooze's *"can't be knocked
//!     prone"* read here without this module having to know it exists.
//!
//! ## What is deliberately not here
//!
//! **Thin ice.** The entry below slippery ice in the same table is a
//! weight budget — *"3d10 × 10 pounds per 10-foot-square area"* — and
//! the engine weighs nothing. It is named here rather than silently
//! skipped because the two entries look like a pair and are not: one is
//! a rule about a turn, the other a rule about a party crossing a lake.

use crate::conditions::{Condition, ConditionTimer};
use crate::engine::encounter::EncounterInstance;
use crate::engine::terrain::TerrainType;
use crate::engine::types::{AbilityScoreType, Coordinate};
use crate::engine::util::footprint_tiles;

/// The DC of the save slippery ice asks for.
///
/// Flat, and that is the point: RAW prints `10` with no scaling clause
/// and nobody to scale it off. Every other save in the engine is keyed
/// to a caster — a spell save DC, a monster's trait DC — and this one
/// has no caster, which is the whole argument in
/// [the module docs](self) for why ice is terrain rather than a zone,
/// stated as a number.
pub const SLIPPERY_ICE_SAVE_DC: i32 = 10;

/// The ability the save is made with.
///
/// Named beside the DC rather than spelled at the one call site so the
/// rule reads as one thing — *"a DC 10 Dexterity saving throw"* — and so
/// the tests that pin it can name it rather than re-deciding it.
pub const SLIPPERY_ICE_SAVE_ABILITY: AbilityScoreType = AbilityScoreType::Dexterity;

impl EncounterInstance {
    /// SRD 5.2 **Slippery Ice**: resolve the footing of whatever is
    /// standing on `actor_id`'s tile, if this turn has not already asked
    /// it.
    ///
    /// Called from [`EncounterInstance::touch_ground`] and nowhere else,
    /// which is what keeps the two RAW triggers honest — see
    /// [the module docs](self).
    pub fn test_footing(&mut self, actor_id: usize) {
        // The body on the board, not the passenger on top of it.
        let body_id = self.movement_body(actor_id);
        if self.footing_checks_this_turn.contains(&body_id) {
            return;
        }
        if !self.footing_at_risk(body_id) {
            return;
        }
        self.footing_checks_this_turn.insert(body_id);
        let name = self.actor_name(body_id);
        let outcome = self.roll_save_vs_condition(
            body_id,
            SLIPPERY_ICE_SAVE_ABILITY,
            SLIPPERY_ICE_SAVE_DC,
            Condition::Prone,
        );
        if outcome.passed() {
            self.log(format!("  slippery ice: {} keeps their feet.", name));
            return;
        }
        if let Some(a) = self.actors.get_mut(&body_id) {
            a.add_condition(Condition::Prone, ConditionTimer::Permanent);
        }
        self.log(format!("  slippery ice: {} goes down.", name));
        // A mount that goes down takes its rider's turn with it in
        // every way the engine already models — the rider is on the
        // mount's tile and shares its Prone-adjacent geometry — so
        // nothing further is installed here. RAW's optional "you must
        // succeed on a DC 10 Dexterity saving throw or fall off" is a
        // Mounted Combat clause about being *moved against its will*,
        // which slipping is not.
    }

    /// True if `actor_id` has footing here that the ice could take.
    ///
    /// Every gate [`EncounterInstance::test_footing`] applies except the
    /// once-per-turn ledger, which is not about the creature at all. One
    /// predicate rather than a list of `if`s inside the resolver,
    /// because the status panel has to ask the same question and a panel
    /// that asked a *nearly* identical one would be the kind of bug
    /// nobody reports: a row that promises a save the engine is never
    /// going to roll, or stays silent before one it is.
    ///
    /// The redirect to the mount is the first of the gates and is why
    /// this takes the whole encounter rather than an actor: a knight on
    /// a horse on the ice is not the thing that slips, and the panel a
    /// player is reading while mounted should be showing them the
    /// horse's problem, because the horse's problem is about to be
    /// theirs.
    ///
    /// The four refusals, each a separate sentence of the rule:
    ///
    ///   - **not on the board** (dead, banished, swallowed) — nothing to
    ///     stand anywhere;
    ///   - **not on the floor** (`is_grounded`: flying, `Lifted`,
    ///     burrowed) — RAW's ice is a surface, and a creature over it is
    ///     not on it;
    ///   - **already Prone** — you cannot fall over from the floor;
    ///   - **immune to Prone** — asked here rather than left to
    ///     `add_condition`, so an ooze does not roll a die whose outcome
    ///     it is exempt from either way. The same ordering
    ///     `apply_zone_contact` uses for its size gate.
    pub fn footing_at_risk(&self, actor_id: usize) -> bool {
        self.footing_at_risk_ignoring_ground(actor_id) && self.on_slippery_ground(actor_id)
    }

    /// [`EncounterInstance::footing_at_risk`] with the one clause that
    /// is about the *tile* peeled off: true when this creature is the
    /// kind of thing slippery ice could knock over, wherever it happens
    /// to be standing.
    ///
    /// The pathfinder is the caller that needs the split. Its hazard
    /// question is asked once per walker and then applied to a hundred
    /// candidate tiles, so "does this creature mind ice" and "is there
    /// ice here" have to come apart — see
    /// `EncounterInstance::aversions_of`. Splitting it here rather than
    /// letting the pathfinder assemble its own version is the point:
    /// whatever this exempts, the resolver exempts, and a walker cannot
    /// come to route around a tile that would have cost it nothing.
    pub fn footing_at_risk_ignoring_ground(&self, actor_id: usize) -> bool {
        let body_id = self.movement_body(actor_id);
        let Some(actor) = self.actors.get(&body_id) else {
            return false;
        };
        actor.is_combat_active()
            && actor.is_grounded()
            && !actor.has_condition(Condition::Prone)
            && !self.actor_immune_to_condition(body_id, Condition::Prone)
    }

    /// True if any tile under `actor_id` is slippery.
    ///
    /// Asked of the body that carries it — a rider is standing on its
    /// mount — for the reason [`EncounterInstance::footing_at_risk`]
    /// redirects, and so that the two agree when the panel asks one and
    /// the resolver the other.
    ///
    /// **Any**, where `footprint_is_water` asks **all**, and the
    /// asymmetry is each rule's own. Immersion is a claim about the
    /// whole body being under the surface — a shark with one corner on
    /// the beach is not swimming — while footing is a claim about the
    /// feet: a Large creature straddling the shoreline of a frozen pond
    /// has put a foot on the ice, and that is the foot that goes out
    /// from under it.
    ///
    /// Public because the tests read it directly, and because "is there
    /// ice under this creature" is a question worth being able to ask
    /// without also asking whether the creature could fall over.
    pub fn on_slippery_ground(&self, actor_id: usize) -> bool {
        let Some(actor) = self.actors.get(&self.movement_body(actor_id)) else {
            return false;
        };
        footprint_tiles(actor.location(), actor.size()).any(|tile| {
            self.terrain_at(tile)
                .is_some_and(|t| t.terrain_type.is_slippery())
        })
    }

    /// Freeze every pool on the board — SRD 5.2's slippery ice, laid the
    /// only way a generated map can honestly lay it.
    ///
    /// The generator has no ice pass of its own and deliberately does
    /// not get one. `flood_pools` already knows how to draw a body of
    /// water that reads as a pond — a random walk under a
    /// footprint-sized brush, kept clear of the map edge, sized so it
    /// can still be crossed in a turn — and a frozen pond is that shape
    /// in winter. Retyping is also what keeps every seeded board in the
    /// suite exactly where it was: this draws no dice at all, so a
    /// `--frozen` run and a thawed one are the same map.
    ///
    /// **It freezes around whoever is standing in the water.** A tile
    /// under a creature is left wet, which is the same courtesy
    /// `conjured_terrain` extends when a wall is raised through an
    /// occupied square, and here it is load-bearing rather than polite:
    /// [`crate::engine::actor_gen`] puts aquatic creatures in pools, and
    /// freezing one over would beach a shark on the lid of its own pond
    /// — out of the water it has to breathe, on a tile it has no speed
    /// for. What it leaves instead is a creature-shaped hole in the ice,
    /// which is the right picture and closes the moment the thing in it
    /// swims out.
    ///
    /// Returns the number of tiles frozen, for the caller that wants to
    /// know whether the board had any water to freeze.
    pub fn freeze_pools(&mut self) -> usize {
        let occupied: std::collections::HashSet<Coordinate> = self
            .actors
            .values()
            .flat_map(|a| footprint_tiles(a.location(), a.size()))
            .collect();
        let wet: Vec<Coordinate> = self
            .board_coordinates()
            .filter(|c| !occupied.contains(c))
            .filter(|c| {
                self.terrain_at(*c)
                    .is_some_and(|t| t.terrain_type.is_water())
            })
            .collect();
        let mut frozen = 0usize;
        for coord in wet {
            if self.set_terrain_at(coord, TerrainType::Ice) {
                frozen += 1;
            }
        }
        frozen
    }
}
