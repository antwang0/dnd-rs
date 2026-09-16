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
        let Some(actor) = self.actors.get(&body_id) else {
            return;
        };
        // A creature that is down, out, or off the floor has no footing
        // to lose. Each of the three is a separate sentence and none of
        // them is the ledger's business, so they are asked before it is
        // written to: a flier that crosses the pond has not spent its
        // check, and lands on the ice later in the same turn still
        // owing one.
        if !actor.is_combat_active()
            || !actor.is_grounded()
            || actor.has_condition(Condition::Prone)
        {
            return;
        }
        if !self.on_slippery_ground(body_id) {
            return;
        }
        // Immunity is checked here rather than left to `add_condition`
        // so a creature that cannot be knocked prone does not roll a
        // die whose outcome it is exempt from either way — the same
        // ordering `apply_zone_contact` uses for the size gate.
        if self.actor_immune_to_condition(body_id, Condition::Prone) {
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

    /// True if any tile of `actor_id`'s footprint is slippery.
    ///
    /// **Any**, where `footprint_is_water` asks **all**, and the
    /// asymmetry is each rule's own. Immersion is a claim about the
    /// whole body being under the surface — a shark with one corner on
    /// the beach is not swimming — while footing is a claim about the
    /// feet: a Large creature straddling the shoreline of a frozen pond
    /// has put a foot on the ice, and that is the foot that goes out
    /// from under it.
    ///
    /// Public because the status panel asks it too: a player whose next
    /// step is onto a frozen pond should be able to read that off the
    /// panel rather than infer it from the colour of a glyph, and the
    /// panel and the rule must not be able to disagree about which tiles
    /// count.
    pub fn on_slippery_ground(&self, actor_id: usize) -> bool {
        let Some(actor) = self.actors.get(&actor_id) else {
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
