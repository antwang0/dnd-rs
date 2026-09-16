//! **Burrowing** — the third number on a 5e speed line, and the third
//! place a creature in this engine can be.
//!
//! *"Speed 30 ft., burrow 10 ft."* Ten stat blocks in the roster
//! print one — the Ankheg, the Badger and its Giant cousin, the
//! Bulette, the Dao, the Earth Elemental, the Purple Worm, the
//! Remorhaz, the Umber Hulk and the Xorn — and until this module
//! existed every one of them carried the same apology in its docstring
//! instead of the speed. The Giant Badger's read *"the engine doesn't
//! track separate burrow speed (no underground-terrain awareness), so
//! the burrow half is collapsed to the walking value"*; the Badger's
//! said *"the engine's board has no third dimension for a burrower to
//! use"*; the Purple Worm folded 30 feet of tunnelling onto a surface
//! speed of 50 and the Remorhaz onto 30.
//!
//! Every one of those roundings did the same thing — turned the slowest
//! number on the stat block into part of the fastest — and every one of
//! them threw away the same thing, which was never the speed. **A
//! burrow speed is a position, not a pace.** What a burrowing monster
//! does with it is leave: it drops out of the fight, crosses the room
//! where nobody can touch it, and comes up somewhere else. The number
//! is how long that takes.
//!
//! ## The model
//!
//! One condition — [`Condition::Burrowed`] — and four rules that read
//! it.
//!
//! **1. Total Cover, both ways.** Nothing on the surface can target a
//! creature under the floor, and it can target nothing on the surface.
//! That is `burrow_blocks_targeting`, which is
//! `swallow_blocks_targeting`'s shape exactly and lands at the same
//! gates: the unconditional targeting check in `Action::validate`, and
//! `EncounterInstance::hostility_blocked` for the three reaction
//! dispatchers that never go through it. One consequence is worth
//! naming because it is the whole point of digging in: **no
//! opportunity attacks**, in either direction. Diving underground in
//! front of a knight does not earn you a longsword in the back, and
//! tunnelling past one earns nothing either.
//!
//! **2. No area effect reaches it.** One row in `actors_in_burst`,
//! beside the swallow's, for the same reason that one is there: a
//! Fireball centred on the tile a bulette is under should not cook the
//! bulette.
//!
//! **3. It moves at its burrow speed, through earth only.**
//! `ActorInstance::base_speed_now` returns the burrow speed while the
//! condition is held, and the pathfinder refuses any tile that is not
//! [`TerrainType::is_diggable`] — which is every tile with ground under
//! it and no others. Water is the one that matters in play: a burrow
//! speed is not a swim speed, and a pool is a wall to something
//! tunnelling toward it.
//!
//! **4. Tremorsense still finds it.** This one needed no code at all,
//! which is the tell that the sense was already modelled properly:
//! `nonvisual_sense_reaches` gates tremorsense on the *subject* being
//! grounded, and a burrower is as grounded as anything on the board
//! gets. So the ankheg's own 60 feet of tremorsense is what one ankheg
//! uses to find another.
//!
//! ## Getting in and out
//!
//! Two actions — `default_actions::BURROW` and `default_actions::
//! SURFACE` — and both are priced in **movement**, at half the
//! creature's speed, which is what `StandUp` charges for the other
//! change of posture the game prices this way.
//!
//! RAW prices neither: burrowing in 5e is simply movement, and a
//! creature with a burrow speed uses it the way a creature with a swim
//! speed uses water. The engine cannot say that, because its movement
//! is a scalar budget over a 2-D grid with no third axis to spend it
//! on — "go down" has no edge in the graph. Half the speed is the
//! nearest honest translation: it comes out of the same budget the
//! descent would have, it leaves a creature that dives in able to
//! tunnel the rest of its round, and it makes surfacing-and-swinging a
//! real turn rather than a free one.
//!
//! The price is read off the speed the actor has **right now**, which
//! means digging in costs half the walking speed and digging out costs
//! half the burrowing one. That asymmetry is not a bug and it is the
//! right way round: coming up out of ten feet of earth is the slow
//! half.
//!
//! ## What is deliberately not modelled
//!
//!   - **Burrowing through solid rock.** RAW gives it to two stat
//!     blocks with an explicit clause — *"the worm can burrow through
//!     solid rock at half its burrow speed"* — and withholds it from
//!     the other eight, who need *"loose earth, sand, or mud"*. Modelled
//!     as neither: [`TerrainType::Wall`] stays impassable to a burrower
//!     exactly as it is to a walker. Granting it would hand the
//!     pathfinder a second passability model keyed on the mover, and
//!     would let a bulette tunnel out of a sealed room — which is a
//!     bigger change to what a map *means* than the clause is worth.
//!   - **Passing underneath another creature.** A burrowed actor keeps
//!     its footprint on the occupancy grid, so the rogue standing over
//!     the tunnel is still in the way. The board is one id per subtile
//!     and always has been; the alternative is a second occupancy layer
//!     that every spawn, shove, drag and teleport in the engine would
//!     have to learn about. What the player sees instead is the churned
//!     earth the thing is under, which is an honest picture of a
//!     tunnel shallow enough to be a tunnel.
//!   - **Being buried by a collapsing tunnel**, and every other clause
//!     about what the ceiling of a hole does. There are no holes; there
//!     is a flag.

use crate::conditions::{Condition, ConditionTimer};
use crate::engine::encounter::EncounterInstance;
use crate::engine::types::{Coordinate, Size};
use crate::engine::util::footprint_tiles;

/// The share of a creature's current speed that digging in — or back
/// out — costs, in movement.
///
/// A half, which is `StandUp`'s price and for the same reason: both are
/// a change of posture that RAW folds into movement without naming a
/// number, and half a move is the one fraction the game has already
/// used for that. See the module docs for why a price exists at all.
pub const BURROW_TRANSIT_FRACTION: f32 = 0.5;

/// Once-per-turn ledger key marking that this actor has **changed
/// layers this turn** — dug in, or dug out.
///
/// Not a rule. RAW puts no limit on how often a creature crosses the
/// surface and neither does this engine: `Burrow` and `Surface` cost
/// movement and nothing else, so a bulette with forty feet to spend may
/// legitimately come up, bite, and go back down.
///
/// What the mark exists for is the *AI*, which needs one bit of memory
/// to avoid the one turn that is never right: surfacing because there
/// is no earth left to tunnel through and then immediately digging back
/// into the same tile. See `ai::simple::try_burrow`, which is the only
/// reader.
pub const BURROW_TRANSIT_TAG: &str = "burrow-transit";

impl EncounterInstance {
    /// Can `actor_id` dig in from where it is standing, right now?
    ///
    /// Three questions, in the order that makes the cheap ones cheap:
    ///
    ///   1. **Is it a burrower?** `base_burrow_speed > 0`.
    ///   2. **Is it already under?** Digging twice is not a thing.
    ///   3. **Is there earth under every tile of it?** Measured over
    ///      the whole footprint rather than the anchor, so a Huge worm
    ///      cannot dive in with three quarters of itself over a lake.
    ///      The same shape `footprint_is_water` is asked in for the
    ///      same reason.
    ///
    /// Deliberately **not** gated on being airborne, which is the one
    /// answer here that took a second look. The engine has no landing
    /// lane for innate flight — a creature born with wings is aloft
    /// from `instantiate_creature` until something knocks it down — so
    /// a gate would have made the Dao's burrow speed a number no turn
    /// could ever spend, which is the exact disease this module was
    /// written to cure. `submerge` lands it instead, and charges
    /// nothing for the trip; see there.
    ///
    /// Deliberately silent about the action economy and about
    /// conditions: whether the actor can *afford* the movement is
    /// `Action::cost`'s business, and whether a grapple or a
    /// petrifaction stops it is the shared `zeros_movement` gate's. This
    /// answers only the question those two cannot — whether the floor
    /// will take it.
    pub fn can_submerge(&self, actor_id: usize) -> bool {
        let Some(actor) = self.actors.get(&actor_id) else {
            return false;
        };
        if !actor.can_burrow() || actor.is_burrowed() {
            return false;
        }
        self.footprint_is_diggable(actor.location(), actor.size())
    }

    /// True when every tile of a `size` footprint anchored at `anchor`
    /// has earth under it — the board-side half of `can_submerge`, and
    /// the check the pathfinder makes one tile at a time.
    ///
    /// Out of bounds counts as undiggable, which is the same direction
    /// `footprint_is_water` fails in and the safe one: a missing tile is
    /// not a tile you may tunnel into.
    pub fn footprint_is_diggable(&self, anchor: Coordinate, size: Size) -> bool {
        footprint_tiles(anchor, size).all(|tile| {
            self.terrain_at(tile)
                .is_some_and(|t| t.terrain_type.is_diggable())
        })
    }

    /// RAW's **Total Cover**, for the floor: true when `actor_id` and
    /// `target_id` are on opposite sides of it.
    ///
    /// `swallow_blocks_targeting`'s shape, one layer of the world down,
    /// and with one difference: a swallow has a party that the cover
    /// does *not* separate — the swallower itself, which is the whole
    /// escape route — and a burrow has none. Earth between two
    /// creatures is earth between two creatures, so the predicate is a
    /// bare exclusive-or and has nothing to exempt.
    ///
    /// Two burrowed creatures can reach each other, which is the other
    /// half of that: they are in the same tunnel.
    ///
    /// Not gated on `is_harmful`, for the reason the swallow's is not:
    /// a floor stops a Cure Wounds exactly as firmly as it stops an
    /// arrow.
    pub fn burrow_blocks_targeting(&self, actor_id: usize, target_id: usize) -> bool {
        let actor_under = self.is_burrowed(actor_id);
        let target_under = self.is_burrowed(target_id);
        actor_under != target_under
    }

    /// `ActorInstance::is_burrowed` by id, with a missing actor reading
    /// as *on the surface*.
    ///
    /// The id-keyed form every gate in the engine actually wants, and
    /// the failure direction is the one that cannot invent cover: an
    /// actor that has left the board does not get to hide under it.
    pub fn is_burrowed(&self, actor_id: usize) -> bool {
        self.actors.get(&actor_id).is_some_and(|a| a.is_burrowed())
    }

    /// Put `actor_id` under the floor, and say so in the log.
    ///
    /// The single writer of `Condition::Burrowed` in the engine — the
    /// `Burrow` action routes here rather than installing the condition
    /// itself, so the log line and the surfacing twin below stay one
    /// pair rather than two half-pairs in `default_actions`.
    ///
    /// Idempotent, and returns whether anything changed.
    pub fn submerge(&mut self, actor_id: usize) -> bool {
        if !self.can_submerge(actor_id) {
            return false;
        }
        let name = self.actor_name(actor_id);
        if let Some(a) = self.actors.get_mut(&actor_id) {
            a.add_condition(Condition::Burrowed, ConditionTimer::Permanent);
            a.mark_once_per_turn_used(BURROW_TRANSIT_TAG);
            // A controlled descent, for the one creature on the roster
            // that can be in the air and still own a burrow speed.
            //
            // `is_airborne` answers `false` the moment the condition
            // above is held, so `reconcile_altitudes` would otherwise
            // see a Dao at cruising height with nothing holding it up
            // and charge it 3d6 for the trip down. That is the general
            // flying rule — *"deprived of the ability to move, the
            // creature falls"* — and a genie that chose to dive into
            // the floor is not deprived of anything. RAW a flier that
            // lands under its own power takes nothing, and this is that
            // sentence: the altitude is zeroed here, in the same write
            // that puts it underground, so the sweep finds nothing to
            // reconcile.
            a.set_altitude_ft(0);
        }
        self.log(format!("{} digs into the ground and disappears.", name));
        true
    }

    /// Bring `actor_id` back up. The inverse of `submerge`, and the
    /// only way out of the condition.
    ///
    /// Takes no view of where the actor is standing, because there is
    /// nothing to check: it got there by tunnelling, every tile it
    /// tunnelled through had earth in it, and earth is something you can
    /// come up out of. Returns whether anything changed.
    pub fn surface(&mut self, actor_id: usize) -> bool {
        if !self.is_burrowed(actor_id) {
            return false;
        }
        let name = self.actor_name(actor_id);
        if let Some(a) = self.actors.get_mut(&actor_id) {
            a.remove_condition(Condition::Burrowed);
            a.mark_once_per_turn_used(BURROW_TRANSIT_TAG);
        }
        self.log(format!("{} bursts up out of the ground!", name));
        true
    }
}
