//! 5e **Suffocation** — the breath clock, and the four stat blocks that
//! were carrying the word "suffocating" with nothing behind it.
//!
//! SRD 5.2, *Suffocation [Hazard]*, in full:
//!
//! > A creature can hold its breath for a number of minutes equal to 1
//! > plus its Constitution modifier (minimum of 30 seconds) before
//! > suffocation begins. When a creature runs out of breath or is
//! > choking, it gains 1 Exhaustion level at the end of each of its
//! > turns. When a creature can breathe again, it removes all levels of
//! > Exhaustion it gained from suffocating.
//!
//! Three things about that paragraph decided the shape of this module.
//!
//! # It pays out in exhaustion, and the ladder is already here
//!
//! The 2014 rule dropped a creature straight to 0 hit points, which
//! would have needed a bespoke death lane. The 2024 rule spends the
//! ladder in `actors::actor_template` instead — six cumulative rungs
//! ending in death at [`EXHAUSTION_DEATH_TIER`], already wired to
//! checks, speed, attacks, saves and hit point maximum. So the whole of
//! "what suffocation does" is one call to `add_condition(Exhausted, …)`
//! per round, and every downstream consequence arrives for free.
//!
//! It also settles who is exempt without a second cohort. RAW gives
//! Constructs, Undead, Oozes and most Elementals immunity to Exhaustion
//! on the stat block, and `add_condition` bounces an immune creature
//! before the ladder sees it — so a zombie at the bottom of a pool is
//! unbothered because its sheet already says so, not because this
//! module carries a list of things that do not breathe.
//!
//! # "Runs out of breath **or** is choking" is two clocks, not one
//!
//! *Running out of breath* is the slow one: 1 + CON minutes, which
//! [`hold_breath_rounds`] turns into rounds. On the engine's shortest
//! chassis that is five rounds and on a Storm Giant it is fifty — long
//! enough that an immersed creature usually walks out of the water
//! before it matters, which is exactly how RAW reads at a table.
//!
//! *Choking* is the fast one, and it is the half with teeth: the airway
//! is blocked, so the clock is already spent and the first round-end
//! costs a rung. Four SRD stat blocks say it in so many words, and all
//! four were in this bestiary already with the clause struck out and a
//! comment saying the engine had no breath clock:
//!
//!   - **Darkmantle** — *"it covers the target, which has the Blinded
//!     condition and is suffocating while the darkmantle is attached"*.
//!   - **Rug of Smothering** — *"the target has the Blinded and
//!     Restrained conditions, is suffocating, and takes 10 (2d6 + 3)
//!     Bludgeoning damage at the start of each of its turns"*.
//!   - **Gelatinous Cube** — *"an engulfed target is suffocating, can't
//!     cast spells with a Verbal component, has the Restrained
//!     condition"*.
//!   - **Water Elemental** — *"the target has the Restrained condition,
//!     is suffocating **unless it can breathe water**"*.
//!
//! That last clause is why the "unless" is asked at each install site
//! rather than inside [`Condition::Choking`]: a merfolk held under by a
//! water elemental is breathing fine, and a merfolk with a darkmantle
//! wrapped around its head is not. The condition means *your airway is
//! blocked*, full stop; who gets one is the attack's business.
//!
//! # "Removes all levels it gained from suffocating" needs a ledger
//!
//! A creature that surfaces sheds the rungs the water cost it and keeps
//! the ones it walked in with — a barbarian who arrived already tired
//! does not get healed by drowning. Nothing about a bare exhaustion
//! count can tell those apart, so [`ActorInstance`] carries a second
//! number beside it recording how much of the total this module is
//! responsible for. See `ActorInstance::suffocation_exhaustion`.
//!
//! # What is deliberately not here
//!
//!   - **The fish out of water.** Nine SRD stat blocks say "can breathe
//!     only underwater" — the three sharks, the two seahorses, the
//!     piranha and its swarm, and the two octopuses (who then take it
//!     back with an hour of held breath). RAW, the seven without the
//!     hour begin suffocating the moment they are on dry land and are
//!     dead six rounds later.
//!
//!     It is a cut rather than an oversight, and what it is waiting on
//!     is not this module. The engine's map generator has no habitat
//!     model: it will anchor a reef shark on a dungeon floor as
//!     cheerfully as in a pool, so shipping the clause would turn most
//!     shark encounters into a six-round countdown to a corpse nobody
//!     fought. The rule needs two things first — a spawn that puts a
//!     water-breather in water, and a pathfinder that will not walk one
//!     out of it — and both are their own feature. Until then
//!     `UNDERWATER_BREATHING_TAG` carries the half of those stat blocks
//!     that keeps them alive and none of the half that kills them,
//!     which is the safe direction to be wrong in.
//!   - **The four-hour clause.** The Sahuagin's *Limited
//!     Amphibiousness* — "it can breathe air and water, but it needs to
//!     be submerged at least once every 4 hours to avoid suffocating
//!     outside water" — is amphibious for the length of any fight, and
//!     is on the exempt list as such. Four hours is 2,400 rounds.
//!   - **Drowning as a way to die.** It is, but only through the ladder:
//!     the sixth rung is death and the module has no shortcut to it.
//!     A CON 10 creature held under from full needs ten rounds to start
//!     the count and six more to finish it, which is longer than most
//!     encounters and is the honest reading of the rule.
//!
//! [`EXHAUSTION_DEATH_TIER`]: crate::actors::actor_template::EXHAUSTION_DEATH_TIER
//! [`ActorInstance`]: crate::actors::actor_template::ActorInstance
//! [`Condition::Choking`]: crate::conditions::Condition::Choking

/// Rounds in one minute of game time. 5e's round is six seconds, so
/// ten — the conversion every "for N minutes" duration in the game
/// passes through, and the one this module needs because RAW prices
/// held breath in minutes and the engine counts in rounds.
pub const ROUNDS_PER_MINUTE: u32 = 10;

/// RAW's floor: *"(minimum of 30 seconds)"*, which is five rounds.
///
/// It binds for exactly the creatures the parenthesis was written for —
/// anything with a Constitution modifier of −1 or worse, where the
/// literal `1 + CON` would be zero minutes or a negative number of
/// them. On this roster that is the commoner tier and the frail
/// undead, and without the floor a CON 8 commoner would begin
/// suffocating the instant its head went under.
pub const MIN_HOLD_BREATH_ROUNDS: u32 = 5;

/// How long `con_mod` can hold its breath, in rounds.
///
/// RAW is *"a number of minutes equal to 1 plus its Constitution
/// modifier (minimum of 30 seconds)"*. The minimum applies to the
/// finished duration rather than to the modifier, which is why the
/// clamp is on the way out and not on the way in: a CON modifier of −3
/// gives −2 minutes, and RAW's answer to that is thirty seconds, not
/// "treat the modifier as zero and give it a minute".
///
/// Saturating on both sides for the same reason every other arithmetic
/// helper in the engine is: an ability score can be buffed and drained
/// by a dozen effects, and a number this is multiplied by ten has no
/// business being the one that panics.
pub fn hold_breath_rounds(con_mod: i32) -> u32 {
    let minutes = con_mod.saturating_add(1).max(0) as u32;
    minutes
        .saturating_mul(ROUNDS_PER_MINUTE)
        .max(MIN_HOLD_BREATH_ROUNDS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_minute_of_breath_is_ten_rounds_plus_one_per_point_of_constitution() {
        // CON 10 (+0) is RAW's bare minute.
        assert_eq!(hold_breath_rounds(0), 10);
        // Every point of modifier buys another minute.
        assert_eq!(hold_breath_rounds(1), 20);
        assert_eq!(hold_breath_rounds(3), 40);
        // A Storm Giant (CON 28, +9) holds its breath for ten minutes.
        assert_eq!(hold_breath_rounds(9), 100);
    }

    #[test]
    fn the_thirty_second_floor_catches_the_frail() {
        // -1 is the first modifier where 1 + CON is worth less than the
        // floor: one minute of breath minus one is zero minutes.
        assert_eq!(hold_breath_rounds(-1), MIN_HOLD_BREATH_ROUNDS);
        // And it keeps catching them all the way down, rather than
        // wrapping through zero into a very long swim.
        assert_eq!(hold_breath_rounds(-5), MIN_HOLD_BREATH_ROUNDS);
        assert_eq!(hold_breath_rounds(i32::MIN), MIN_HOLD_BREATH_ROUNDS);
    }

    #[test]
    fn an_absurd_constitution_does_not_overflow_the_clock() {
        // Nothing on the roster is near this; the point is that a
        // stacked pile of ability buffs cannot make the breath clock
        // the thing that panics.
        assert!(hold_breath_rounds(i32::MAX) > 0);
    }
}
