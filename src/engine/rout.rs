//! **The rout** — SRD 5.2's compulsion to run, which the engine could
//! describe and never enforce.
//!
//! Five effects in the book add the same second sentence to a fear, and
//! four of them are in this engine:
//!
//! > **Fear** (level 3). *"A Frightened creature takes the Dash action
//! > and moves away from you by the safest route on each of its turns
//! > unless there is nowhere to move."*
//! >
//! > **Turn Undead** (Channel Divinity). *"For that duration, it tries
//! > to move as far from you as it can on its turns."*
//! >
//! > **Compulsion** (level 4). *"…use a Bonus Action on subsequent
//! > turns to designate a direction"* — whose own docstring named this
//! > lane as the reason its forced-movement clause was absent.
//! >
//! > **Antipathy/Sympathy** (level 8). *"The Frightened creature must
//! > use its movement on its turns to get as far away as possible from
//! > the target, moving by the safest route."*
//!
//! None of that is the Frightened condition. RAW's condition is two
//! clauses — Disadvantage while the source is in sight, and no
//! *willing* step toward it — and both are refusals. A frightened
//! creature that plants its feet and keeps swinging is obeying every
//! word of it, which is what every frightened creature in this engine
//! did: a cleric's Turn Undead moved nothing, and the Fear spell was a
//! cone of Disadvantage.
//!
//! [`Condition::Routed`] is the missing sentence, and
//! `EncounterInstance::run_rout` is where it happens: at the top of the
//! holder's turn, in the start-of-turn run beside the Conquest aura and
//! the drifting clouds, the engine spends the creature's movement
//! putting distance between it and whatever it is running from.
//!
//! ## Why the engine moves it rather than the AI
//!
//! Because RAW does. *"It tries to move as far from you as it can"* is
//! not advice to the creature — it is a thing that happens to it, and
//! the player whose paladin is running from a lich is not being asked
//! either. Every other compulsion the engine has already works this way
//! (the Conquest aura zeroes a turn's movement, Earthbind walks a flier
//! down, a strong wind grounds it), and all of them live in the same
//! start-of-turn run. An AI rung would also have been wrong twice over:
//! it would leave a player-controlled victim exempt, and it would be
//! advice the AI could decline.
//!
//! ## The walk
//!
//! One greedy step at a time, taking whichever neighbouring anchor most
//! increases the gap to the source, until the movement runs out or no
//! step improves on standing still. Three things make that the right
//! shape rather than a shortcut:
//!
//!   - **It is what "as far as it can" means on a board with walls on
//!     it.** A creature backed into a corner has nowhere to go and RAW
//!     says so outright (*"unless there is nowhere to move"*); a greedy
//!     walk discovers that by running out of improving steps, which is
//!     the same answer a search would give and costs a neighbourhood
//!     scan instead of a flood fill.
//!   - **Each step is a real step.** It goes through `MoveActor`, so
//!     leaving a threatened square provokes, a web on the way out bites,
//!     and the ice underfoot is tested — all of which RAW charges a
//!     fleeing creature for. "The safest route" is the one clause this
//!     gives up: the engine's pathfinder has a hazard-avoiding mode and
//!     this does not use it, because a rout that routed *around* the
//!     wall of fire between it and the door would be reasoning the
//!     panicking creature is not doing.
//!   - **It cannot walk closer.** Every step strictly increases the gap,
//!     so the rout never fights the Frightened clause it arrives beside
//!     — `Move::custom_validate_input` would refuse such a step anyway,
//!     and this way the two rules never have to meet.
//!
//! ## What it is not
//!
//! **Not a Dash.** RAW's Fear says the creature *takes the Dash
//! action*, doubling the ground it covers. The rout spends the turn's
//! ordinary movement and leaves the Action alone, which is the
//! conservative direction and the one that keeps the lane shared: Turn
//! Undead's version grants no Dash, Antipathy's grants no Dash, and a
//! lane that Dashed for all four would be over-reading three of them.
//! Fear's extra thirty feet is the divergence, and it is named here
//! rather than in the spell.
//!
//! **Not the whole turn.** The creature still acts afterwards. That is
//! RAW for Antipathy and Compulsion, and an under-read of Fear (whose
//! Dash *is* the Action). Turn Undead's own Incapacitated clause is
//! what takes a turned undead's Action away, and it arrives as a
//! condition rather than through this module.

/// The eight neighbouring anchors, in a fixed order so a seeded replay
/// breaks ties the same way twice.
///
/// Declaration order is the tie-break and nothing else: the walk sorts
/// by the gap a step buys, and only steps that buy the *same* gap fall
/// through to this. Cardinals first, so a creature with a clear line
/// directly away from the thing it fears takes it rather than drifting
/// diagonally across the room — the same preference `steps_toward`
/// expresses for the walk in the other direction.
pub const FLIGHT_STEPS: [(isize, isize); 8] = [
    (-1, 0),
    (1, 0),
    (0, -1),
    (0, 1),
    (-1, -1),
    (-1, 1),
    (1, -1),
    (1, 1),
];

/// The most tiles a single rout will walk before giving up, whatever
/// the movement budget says.
///
/// Not a rule — a backstop. The walk terminates on its own because
/// every step spends movement and every step must strictly increase a
/// bounded quantity, but both of those are invariants of code rather
/// than of the type system, and a rout is run inside the turn pipeline
/// where a non-terminating loop is a hung game rather than a failed
/// assertion. Forty tiles is a hundred feet, which is longer than any
/// speed in the bestiary and wider than the board the binary ships.
pub const MAX_FLIGHT_STEPS: usize = 40;
