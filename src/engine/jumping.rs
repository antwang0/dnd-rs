//! SRD 5.2 **Jumping** — the rule that turns a hole in the floor from a
//! wall into a decision.
//!
//! > **Long Jump.** When you make a Long Jump, you leap horizontally a
//! > number of feet up to your Strength score if you move at least 10
//! > feet immediately before the jump. When you make a standing Long
//! > Jump, you can leap only half that distance. Either way, each foot
//! > you jump costs a foot of movement.
//! >
//! > If you land in Difficult Terrain, you must succeed on a DC 10
//! > Dexterity (Acrobatics) check or have the Prone condition.
//! >
//! > This Long Jump rule assumes that the height of the jump doesn't
//! > matter, such as a jump across a stream or chasm.
//!
//! The engine had none of it, and could not have used it if it had: a
//! board with no gaps in it is a board where a jump is a walk that
//! costs the same. The rule and the terrain arrive together, and each is
//! why the other is worth having — see [`TerrainType::Chasm`], the tile
//! that is impassable and crossable at once.
//!
//! # A jump is an edge, not an action
//!
//! The whole of this feature reaches the game through one place: the
//! jump lane in `EncounterInstance::dijkstra_path`, which the search
//! walks beside its ordinary eight-neighbour step. A hop is a graph edge
//! from the near lip to the far one, priced at the distance it covers,
//! and every constant and predicate it needs is in this module.
//!
//! The alternative was a `Jump` action, and it would have been the
//! larger change and the smaller feature. Everything in the engine that
//! asks *"can this creature get there, and what does it cost"* — the
//! AI's approach, the reach checks that price a move, the player's click
//! on a far tile, `MoveActor`'s per-step walk — asks it through
//! `path_to`. An action would have had to be aimed, chosen, and taught
//! to each of those in turn; an edge is picked up by all of them without
//! any of them knowing the rule exists.
//!
//! # The one place the edge is an approximation
//!
//! RAW's running start is a fact about *the route*, not about the tile —
//! and Dijkstra settles tiles. The lane reads the run off the search
//! tree (`parent` backwards, four tiles, same direction), which is the
//! movement made immediately before the hop on the **cheapest** route to
//! the takeoff tile; a dearer route that happened to arrive running is
//! not considered, because the cheaper one settled the tile first.
//!
//! That is a real gap and it is one-directional: it can only withhold a
//! jump that a slightly longer approach would have earned, never grant
//! one nobody ran for. Closing it means carrying the incoming direction
//! in the search state, which multiplies the node count by eight for a
//! case that needs the near lip to be reachable two ways with different
//! approach directions *and* the cheaper of the two to be the one that
//! turns. On a board whose rifts are straight cracks in open floor,
//! backing up and running at it is the same route the search already
//! found.
//!
//! # What a hop may not do
//!
//! A jump adds routes the walk could not take, and *only* those. Every
//! tile a hop passes over must be a gap ([`footprint_clears_as_air`]),
//! so a jump can never be a way to skip a web, an opportunity attack, or
//! the stretch of Spike Growth a step would have paid for. The landing
//! is an ordinary tile entered in the ordinary way, so everything that
//! happens to a creature arriving somewhere — the zone it touches, the
//! loot it picks up, the trap under it — happens to a jumper too.
//!
//! # The height that is not modelled
//!
//! RAW's High Jump, and RAW's *"you must succeed on a DC 10 Strength
//! (Athletics) check to clear a low obstacle"*, are both absent, and
//! both for the same reason: the board is flat. Altitude in this engine
//! is a scalar attached to a creature (`crate::engine::falling`), not a
//! third coordinate, so there is no height for a High Jump to reach and
//! no obstacle profile for a hedge to have. What the Long Jump rule
//! itself says about that is the licence for the omission — *"this Long
//! Jump rule assumes that the height of the jump doesn't matter"* — and
//! a jump across a chasm is precisely the case it names.

use crate::conditions::condition_template::{Condition, ConditionTimer};
use crate::engine::encounter::EncounterInstance;
use crate::engine::terrain::TerrainType;
use crate::engine::types::{AbilityScoreType, Coordinate, Size, Skill};
use crate::engine::util::{feet_from_tiles, get_tiles_from_size, tiles_from_feet};

/// A stat block's own Long Jump clause — the three-shaped family SRD
/// 5.2 writes on a dozen creatures, and one struct because the three
/// shapes differ in exactly one bit.
///
/// | printed as | who has it | distance | running start |
/// |---|---|---|---|
/// | **Standing Leap** | frog, giant frog, giant toad | 10–20 ft | not needed |
/// | **Leap** (Bonus Action) | bulette, giant ape, half-dragon, hezrou, lamia | 30 ft | not needed |
/// | **Running Leap** | lion, tiger | 25 ft | **required** |
///
/// Every one of them is *"the creature's Long Jump is up to N feet"*
/// with a clause about the run-up, which is why they are one field and
/// not three. They **replace** the Strength rule rather than adding to
/// it — see `ActorInstance::long_jump_feet`, which takes the larger of
/// the two and so never punishes a strong creature for also having the
/// trait.
///
/// The bonus-action **Leap** is the one collapse in the table. RAW
/// prints it as an action costing ten feet of movement for a thirty-foot
/// jump; here it is a thirty-foot Long Jump at the ordinary
/// foot-per-foot price, which is the same trade
/// [`Condition::Leaping`][crate::conditions::condition_template::Condition::Leaping]
/// makes for the Jump spell and for the same reason — a jump is an edge
/// in a Dijkstra search, and a discounted, once-a-turn edge would have
/// to be a second dimension of the search state.
///
/// Only the Long Jump is carried. The High Jump the same stat blocks
/// print has nothing to clear: the board is flat and altitude is a
/// scalar (`crate::engine::falling`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Leap {
    /// The Long Jump distance the clause names, in feet.
    pub feet: u32,
    /// `true` for **Running Leap**, whose RAW opens *"With a 10-foot
    /// running start"*; `false` for the two clauses that say *"with or
    /// without a running start"* or do not mention one.
    ///
    /// The whole of what separates the three rows above, and the reason
    /// this is a bit rather than a second field: a creature that needs
    /// the run-up and does not have it falls back to the Strength rule,
    /// which is exactly what RAW's silence means.
    pub needs_running_start: bool,
}

impl Leap {
    /// **Standing Leap** and the bonus-action **Leap** — a distance that
    /// does not ask for a run-up.
    pub const fn standing(feet: u32) -> Self {
        Self { feet, needs_running_start: false }
    }

    /// **Running Leap** — *"With a 10-foot running start, the lion can
    /// Long Jump up to 25 feet."*
    pub const fn running(feet: u32) -> Self {
        Self { feet, needs_running_start: true }
    }

    /// What this clause is worth to a jump made with (or without) a
    /// running start — `0` when it asks for one and there is none.
    pub fn feet_with(self, running_start: bool) -> u32 {
        if running_start || !self.needs_running_start {
            self.feet
        } else {
            0
        }
    }
}

/// SRD 5.2 Long Jump's *"if you move at least 10 feet immediately
/// before the jump"*, in tiles on the 2.5-ft grid.
///
/// In tiles because the only thing that reads it counts a run in tiles,
/// the same way every charge clause in the bestiary does: a diagonal
/// tile is worth three and a half feet and is credited as one, and that
/// rounding has stood behind every pounce in the engine since
/// `ActorInstance::straight_run_tiles` was written.
pub const RUNNING_START_TILES: isize = tiles_from_feet(10) as isize;

/// The longest hop the jump lane will consider, in tiles.
///
/// A guard on the loop rather than a rule. The Strength allowance and
/// the movement budget both cut a jump off long before this, and the
/// only caller that can reach it is a flier, whose distance allowance is
/// unbounded by design. A hundred feet is further than anything in the
/// book moves in a turn, so the budget is always the binding constraint
/// — this is here so that "unbounded" never means "unbounded loop".
pub const MAX_JUMP_TILES: u32 = 40;

/// SRD 5.2 Long Jump's *"succeed on a DC 10 Dexterity (Acrobatics)
/// check or have the Prone condition"*.
pub const LANDING_DC: i32 = 10;

/// The eight directions a Long Jump can be made in.
///
/// The same neighbourhood the walk lane uses, unrolled, because the jump
/// lane needs each direction as a pair it can compare against a straight
/// run rather than as two loop variables with a hole in the middle.
pub const JUMP_DIRECTIONS: [(isize, isize); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];

/// True when a body of `size` anchored at `coord` is over **nothing but
/// empty air it could clear** — the midflight test every tile a hop
/// passes over has to pass.
///
/// Three clauses, and each of them is a different thing a hop could
/// otherwise cheat at:
///
///   - **At least one tile is a gap.** Without it, a "jump" over
///     ordinary floor would be a teleport past everything a walk is
///     charged for. This is the clause that makes the jump lane strictly
///     additive: it can only reach places the walk could not.
///   - **No tile is solid.** You do not leap over a wall, over a pane of
///     force, or off the edge of the map. `is_passable` and `is_gap` are
///     deliberately different questions — see [`TerrainType::is_gap`] —
///     and this is where the difference is spent.
///   - **No other creature is underneath.** 5e's movement rules do not
///     grant a hop over somebody's head, and allowing one would have
///     made a line of defenders along a rift worth nothing. The jumper's
///     own footprint is exempt, because the first tile of any hop
///     overlaps the tile it took off from — which is the whole reason
///     this takes a `mover_id` instead of reusing `can_move_to`'s
///     occupancy test.
pub fn footprint_clears_as_air(
    ei: &EncounterInstance,
    mover_id: usize,
    coord: Coordinate,
    size: Size,
) -> bool {
    let span = get_tiles_from_size(size) as isize;
    let mut saw_gap = false;
    for dy in 0..span {
        for dx in 0..span {
            let tile = Coordinate::new(coord.x + dx, coord.y + dy);
            let Some(info) = ei.terrain_at(tile) else {
                return false;
            };
            if info.terrain_type.is_gap() {
                saw_gap = true;
            } else if !info.terrain_type.is_passable() {
                return false;
            }
            if ei.actor_id_at(tile).is_some_and(|id| id != mover_id) {
                return false;
            }
        }
    }
    saw_gap
}

/// SRD 5.2 Long Jump's landing clause, resolved for a hop that has
/// already happened: *"If you land in Difficult Terrain, you must
/// succeed on a DC 10 Dexterity (Acrobatics) check or have the Prone
/// condition."*
///
/// Called by `side_effects::MoveActor` after every step of a walked
/// path. A one-tile step returns on the first comparison, so the
/// ordinary walk pays almost nothing for this living on the hot path.
///
/// **It re-derives that the step was a leap rather than trusting its
/// length**, and that is worth the footprint sweep it costs. A long
/// step is a jump *in practice* — the walk lane only ever queues
/// neighbours — but "in practice" is not a guarantee: `Move` falls back
/// to a bare `vec![destination]` when the pathfinder comes back empty,
/// and the suite is full of `MoveActor`s hand-built with one long step
/// to set a board up. Reading a leap off the length alone would have
/// printed "leaps 25 ft across the gap" for every one of them, and
/// rolled an Acrobatics check at the end of it. So the test is the
/// same one the edge was built on: a clean straight line, over nothing
/// but air the creature could clear. Anything else is somebody moving a
/// creature, and this rule has nothing to say about it.
///
/// The check is read off the *anchor* tile rather than the whole
/// footprint. A Huge body coming down astride the boundary between rubble
/// and flagstone has some of itself on each, and RAW has no rule for
/// splitting the difference; the anchor is the tile the engine already
/// treats as where a creature is.
///
/// **No waiver for Land's Stride.** A creature that
/// `ignores_difficult_terrain` pays no movement surcharge for rubble and
/// still lands in it: RAW's waiver is priced in feet — *"moving through
/// nonmagical difficult terrain costs you no extra movement"* — and says
/// nothing about footing. Reading it as also cancelling this check would
/// be extending a clause rather than applying one.
pub fn resolve_landing(ei: &mut EncounterInstance, actor_id: usize, from: Coordinate) {
    let Some(to) = ei.actors.get(&actor_id).map(|a| a.location()) else {
        return;
    };
    let delta = to - from;
    let tiles = delta.x.abs().max(delta.y.abs());
    if tiles <= 1 {
        return;
    }
    let unit = Coordinate::new(delta.x.signum(), delta.y.signum());
    // A straight line along one of the eight directions — anything else
    // is not a shape the jump lane can produce.
    if delta != Coordinate::new(unit.x * tiles, unit.y * tiles) {
        return;
    }
    let Some(size) = ei.actors.get(&actor_id).map(|a| a.size()) else {
        return;
    };
    let cleared = (1..tiles).all(|k| {
        footprint_clears_as_air(
            ei,
            actor_id,
            from + Coordinate::new(unit.x * k, unit.y * k),
            size,
        )
    });
    if !cleared {
        return;
    }
    let name = ei.actor_name(actor_id);
    ei.log(format!(
        "  {} leaps {} ft across the gap.",
        name,
        feet_from_tiles(tiles as u32)
    ));
    if !ei
        .terrain_at(to)
        .is_some_and(|t| t.terrain_type == TerrainType::DifficultTerrain)
    {
        return;
    }
    let roll = ei.roll_ability_check(actor_id, AbilityScoreType::Dexterity, Some(Skill::Acrobatics));
    if roll >= LANDING_DC {
        ei.log(format!(
            "  {} lands on broken ground: acrobatics {} vs DC {} \u{2014} kept their feet.",
            name, roll, LANDING_DC
        ));
        return;
    }
    ei.log(format!(
        "  {} lands on broken ground: acrobatics {} vs DC {} \u{2014} goes down.",
        name, roll, LANDING_DC
    ));
    if let Some(a) = ei.actors.get_mut(&actor_id) {
        a.add_condition(Condition::Prone, ConditionTimer::Permanent);
    }
}
