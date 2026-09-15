//! **Incorporeal Movement** — the clause that makes the room's walls
//! not the ghost's walls.
//!
//! *"The ghost can move through other creatures and objects as if they
//! were Difficult Terrain. It takes 5 (1d10) Force damage if it ends
//! its turn inside an object."*
//!
//! Eight stat blocks in the roster print it — the Allip, the Banshee,
//! the Ghost, the Poltergeist, the Shadow, the Shadow Demon, the
//! Specter and the Wraith — and every one of them carried the same
//! apology instead. The Allip's was the longest and said the most:
//! the clause was *"omitted as a deliberate scope cut — the engine's
//! pathfinding doesn't yet model wall-phasing actors, and modeling the
//! half-speed-through-walls clause would require a per-tile
//! 'incorporeal cost' lane that doesn't exist"*, and the
//! bludgeoning/piercing/slashing resistance was offered as *"the
//! load-bearing 'hard to hit with physical weapons' half of the
//! incorporeal identity."*
//!
//! It is not the load-bearing half. Resistance is arithmetic; this is
//! the fight. A spirit that can cross the stone reaches the back rank
//! on turn one, ignores the corridor the party chose to hold, and
//! leaves through the wall it came in by — and the counterplay RAW
//! gives is not a better sword, it is making the thing *stop* somewhere
//! it can be reached.
//!
//! ## The three rules
//!
//! **1. Stone is a tile it may enter.** One arm of
//! `EncounterInstance::tile_admits`, which is the single subtile
//! chokepoint every mover in the engine already asks — so the walk, the
//! AI's directional BFS, a shove, a drag and a teleport all learn the
//! rule at once and none of them has to know it exists. Only
//! [`TerrainType::is_phaseable`] widens: a `Wall`. A chasm is the
//! floor's absence rather than an object, and a Wall of Force is the
//! one barrier 5e names as stopping exactly this kind of travel.
//!
//! **2. It costs double.** RAW's *"as if they were Difficult
//! Terrain"*, priced in `dijkstra_path` — and pointedly *not* through
//! the difficult-terrain waivers. Freedom of Movement and Land's
//! Stride waive a property of the ground; the stone a spirit is
//! pushing through is not the ground, and a ghost that could phase for
//! free would have no reason ever to be in the open.
//!
//! **3. Stopping inside costs 1d10 Force.** `tick_incorporeal_lodging`,
//! at round-end, which is the engine's *"at the end of each of its
//! turns"* lane — the one the repeated saves, the staged saves and the
//! damage-over-time table already share.
//!
//! The third rule is the one that makes the first two a fight rather
//! than an escape hatch. A wall is total cover in both directions: the
//! engine's line of sight stops dead at one, so a spirit that ends its
//! turn in the stone cannot be shot and cannot shoot, exactly as RAW
//! intends — and it pays five hit points a round for the privilege.
//! That price is what stops the lane being a place to hide, and it is
//! why the force damage shipped in the same change as the phasing
//! rather than after it.
//!
//! ## What is deliberately not modelled
//!
//!   - **Moving through other creatures.** The other half of RAW's own
//!     sentence. The board is one actor id per subtile — `actor_map`
//!     has been that since it existed — so two creatures sharing a
//!     square is a second occupancy layer rather than a predicate, and
//!     every spawn, shove, drag, growth and teleport in the engine
//!     would have to learn about it. `ActorInstance::phases_through_objects`
//!     is named for the half that is here rather than for the sentence
//!     it comes from, so no caller can read more into it than it says.
//!   - **The Ethereal Plane.** RAW's incorporeal undead mostly also
//!     have a way onto it; the engine has one plane and
//!     `crate::engine::banishment` for the things that leave it.
//!   - **Ending inside a *creature***, which RAW charges the same 1d10
//!     for. Same reason as the first bullet: it cannot happen here.

use crate::engine::dice::Dice;
use crate::engine::encounter::EncounterInstance;
use crate::engine::types::DamageType;
use crate::engine::util::get_tiles_from_size;

/// What a tile of solid object costs a spirit pushing through it, as a
/// multiplier on the ordinary step — RAW's *"as if they were Difficult
/// Terrain"*.
///
/// The same `2.0` [`crate::engine::terrain::TerrainType::movement_cost`]
/// charges for rubble and for swimming, and named separately from it
/// because it is charged separately: a `Wall`'s own `movement_cost` is
/// `1.0` (nothing walks into one), and this is the only place the
/// number is ever applied to one.
pub const PHASE_COST_MULTIPLIER: f32 = 2.0;

/// The die RAW rolls against a spirit that ended its turn in the stone
/// — *"5 (1d10) Force damage"*.
pub const LODGED_DAMAGE: Dice = Dice::new(1, 10);

impl EncounterInstance {
    /// True when any tile of `actor_id`'s footprint is inside a solid
    /// object — RAW's *"inside an object"*, measured the way every
    /// other footprint question in the engine is.
    ///
    /// **Any** tile, not all of them: a Large wraith with one corner in
    /// the wall is a wraith that is partly in a wall, and RAW does not
    /// grade the sentence. The direction also matters for what the
    /// clause is *for* — a spirit that could park most of itself in the
    /// stone and pay nothing would have found the hiding place the
    /// damage exists to close.
    pub fn is_lodged_in_stone(&self, actor_id: usize) -> bool {
        let Some(actor) = self.actors.get(&actor_id) else {
            return false;
        };
        let anchor = actor.location();
        let width = get_tiles_from_size(actor.size()) as isize;
        (0..width).any(|dx| {
            (0..width).any(|dy| {
                self.terrain_at(anchor + crate::engine::types::Coordinate::new(dx, dy))
                    .is_some_and(|t| t.terrain_type.is_phaseable())
            })
        })
    }

    /// SRD 5.2's *"it takes 5 (1d10) Force damage if it ends its turn
    /// inside an object."*
    ///
    /// Called from `round_end`, which is the engine's end-of-turn lane
    /// — the same moment the repeated saves, the staged saves and the
    /// condition damage-over-time table are resolved at.
    ///
    /// Gated on the creature carrying the trait, not merely on standing
    /// in stone. Nothing else can be there, so the gate is belt and
    /// braces; it is written anyway because the thing that *could* put
    /// something else in a wall is a terrain write — a Wall of Stone
    /// conjured over somebody's head — and a creature entombed by a
    /// spell should not be quietly billed under a ghost's rule while
    /// nothing else about being sealed in is modelled.
    ///
    /// Force, which is deliberate and is the one damage type none of
    /// the eight resists: RAW gives the incorporeal undead
    /// bludgeoning/piercing/slashing resistance and often necrotic
    /// immunity, and then charges them for lodging in a type their own
    /// stat block has no answer to. The clause is written to be paid.
    pub fn tick_incorporeal_lodging(&mut self, actor_id: usize) {
        let carries = self
            .actors
            .get(&actor_id)
            .is_some_and(|a| a.phases_through_objects() && a.is_combat_active());
        if !carries || !self.is_lodged_in_stone(actor_id) {
            return;
        }
        let amount = self.roll(&LODGED_DAMAGE);
        let name = self.actor_name(actor_id);
        self.log(format!(
            "  {} is lodged in solid stone: 1d10({}) force.",
            name, amount
        ));
        use crate::engine::side_effects::{ApplicableSideEffect, DealDamage};
        DealDamage {
            actor_id,
            amount,
            damage_type: DamageType::Force,
        }
        .apply(self);
    }
}
