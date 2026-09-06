//! 5e **swarms** — the six SRD statblocks that are one initiative slot
//! wearing hundreds of bodies.
//!
//! A swarm is not a big monster. It is a Medium-sized cloud of Tiny
//! ones, and everything odd about its statblock follows from that:
//!
//!   - **It thins as it dies.** Every one of them writes "…or half
//!     as much damage if the swarm has half of its hit points or fewer"
//!     into its Bites line, because half the mouths are gone. The rule
//!     lives once, on `ActorInstance::is_thinned_swarm`, read at
//!     `attack::attacker_scoped_damage_reduction`.
//!   - **It cannot be healed.** "The swarm can't regain hit points or
//!     gain temporary hit points" — the dead bats are dead. Enforced at
//!     `heal` / `gain_temp_hp`, the two chokepoints every source of
//!     either goes through.
//!   - **It cannot be put in most conditions.** You cannot knock a
//!     cloud prone, grapple it, or paralyse it; there is no *it* to
//!     take hold of. Plain `condition_immunities` rows.
//!   - **A blade barely works on it.** Five of the six resist
//!     bludgeoning, piercing and slashing: a sword swung through a
//!     swarm of insects kills the insects it hits and none of the rest.
//!     Area damage is the answer, and that is the tactical question the
//!     whole family exists to ask.
//!
//! Only the rat swarm breaks the resistance pattern, and RAW is
//! deliberate about it: rats are big enough to hit. It is the swarm you
//! can fight with a weapon, which is why it sits at the bottom of the
//! ladder alongside the bats.
//!
//! Not modelled: "the swarm can occupy another creature's space, and
//! vice versa". `EncounterInstance::actor_map` holds one actor id per
//! tile, so a swarm stands beside its victim rather than inside them,
//! and its RAW reach-0 Bites becomes an ordinary melee reach. The
//! substitution costs the swarm nothing it can feel — it is still
//! adjacent, still biting — and multi-occupancy is a change to the
//! board, not to a creature.

use crate::actions::action_template::Action;
use crate::actions::class_features::{BLOOD_FRENZY_TAG, SWIM_SPEED_TAG, UNDERWATER_BREATHING_TAG};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    SWARM_OF_BATS_BITES, SWARM_OF_INSECTS_BITES, SWARM_OF_PIRANHAS_BITES,
    SWARM_OF_RATS_BITES, SWARM_OF_RAVENS_BEAKS, SWARM_OF_VENOMOUS_SNAKES_BITES,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{
    CreatureType, DamageModifier, DamageType, Size, SpecialSense,
};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// The eight conditions no swarm can be put in. RAW lists the same set
/// on all six statblocks, so it is written once here.
///
/// What they have in common is that each one needs a single body to act
/// on: you knock *a creature* prone, grapple *it*, petrify *it*. A cloud
/// of six hundred bats has no such thing — knock one down and the other
/// five hundred and ninety-nine keep flying. Blinded, Deafened and
/// Poisoned are deliberately absent for the same reason in reverse:
/// those are afflictions each individual bat can suffer, and a swarm
/// where every member is blinded is a blinded swarm.
fn swarm_condition_immunities() -> HashSet<Condition> {
    HashSet::from([
        Condition::Charmed,
        Condition::Frightened,
        Condition::Grappled,
        Condition::Paralyzed,
        Condition::Petrified,
        Condition::Prone,
        Condition::Restrained,
        Condition::Stunned,
    ])
}

/// Resistance to the three physical damage types — the "a blade passes
/// through the cloud" clause five of the six swarms carry.
fn swarm_physical_resistances() -> HashMap<DamageType, DamageModifier> {
    HashMap::from([
        (DamageType::Bludgeoning, DamageModifier::Resistance),
        (DamageType::Piercing, DamageModifier::Resistance),
        (DamageType::Slashing, DamageModifier::Resistance),
    ])
}

/// Shared chassis for the six SRD swarm statblocks.
///
/// Every swarm is Medium, `is_swarm`, a Beast, immune to the same eight
/// conditions, and has exactly one Action: its Bites. Those five facts
/// are the whole of what makes a swarm a swarm, so they live here and
/// the per-statblock call sites carry only what actually differs —
/// name, glyph, defences, ability scores, speed, senses, and the bite
/// itself.
///
/// The same "one shared envelope, per-entry swaps" shape as
/// `barbarians::subclass_barbarian_template`, and it has since been
/// asked to prove it: the Swarm of Ravens arrived as one call plus two
/// overrides rather than as a forty-line struct literal that had to
/// remember eight condition immunities.
///
/// `physical_resistance` is a parameter rather than part of the chassis
/// because the rat swarm is the one statblock that doesn't get it, and
/// a shared default the rat swarm then had to subtract would hide the
/// single most tactically-relevant difference between the two CR-¼
/// swarms behind a `..` spread.
#[allow(clippy::too_many_arguments)]
fn swarm_template(
    name: &'static str,
    glyph: char,
    ac: u32,
    hitpoints: &str,
    speed: f32,
    abilities: [u32; 6],
    senses: HashSet<SpecialSense>,
    cr: f32,
    bite: &'static (dyn Action + Send + Sync),
    physical_resistance: bool,
    features: HashSet<&'static str>,
) -> CreatureTemplate {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(bite);
    let [strength, dexterity, constitution, intelligence, wisdom, charisma] = abilities;
    CreatureTemplate {
        name,
        glyph,
        ac,
        hitpoints: hitpoints.parse().unwrap(),
        speed,
        strength,
        dexterity,
        constitution,
        intelligence,
        wisdom,
        charisma,
        senses,
        cr,
        size: Size::Medium,
        creature_type: CreatureType::Beast,
        actions,
        is_swarm: true,
        condition_immunities: swarm_condition_immunities(),
        damage_modifiers: if physical_resistance {
            swarm_physical_resistances()
        } else {
            HashMap::new()
        },
        features,
        ..CreatureTemplate::defaults()
    }
}

/// Swarm of Bats — CR ¼ **Large** swarm of Tiny beasts. RAW: AC 12, 11
/// HP (2d10), speed 5 ft / fly 30 ft, blindsight 60 ft, resistant to
/// bludgeoning / piercing / slashing, Bites 5 (2d4) piercing.
///
/// The blind swarm. Fly 30 and blindsight 60 make it the one that
/// picks its target — it crosses the board, ignores the fog the party
/// hid in, and lands on the caster. Its bite is the lightest on the
/// ladder; the threat is the arrival, not the teeth.
///
/// Note what blindsight now costs the party: since
/// `nonvisual_sense_reaches` reads it at both sight gates, a Swarm of
/// Bats sees straight through Invisibility and Fog Cloud alike inside
/// 24 tiles. The wizard's usual two answers to "something is coming for
/// me" are both off.
///
/// RAW's "5 ft., Fly 30 ft." is written as the two numbers it is. The
/// ravens leave the floor too, and faster, but the bats got here
/// first. It is
/// also the only member of the chassis that overrides anything, which
/// is why the flying speed arrives as a `..swarm_template(…)` spread at
/// the call site rather than as an eighth parameter four other swarms
/// would have to pass a zero to.
///
/// The consequence worth naming is the walking speed: 0. A swarm of
/// bats that is knocked out of the air — the general flying rule, via
/// `flight_is_disabled` — does not crawl to the wizard, it sits on the
/// floor. That is RAW and it is the counterplay this stat block never
/// had while its fly speed was spelled "30 ft. walking".
///
/// Glyph 'ß' — a doubled-up 's', for a swarm that is many of one thing.
/// The six swarms share the mark and differ by name and team colour,
/// which is the right read: on a board, "that is a swarm" is the fact
/// that changes your plan, and which vermin it is made of is detail.
pub static SWARM_OF_BATS_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    CreatureTemplate {
        // RAW speed line: Speed 5 ft., Fly 30 ft. The walking half is
        // the token five feet every airborne swarm on the ladder gets;
        // the swarm's business is done in the air.
        fly_speed: 30.0,
        // The one swarm SRD 5.2 prints as **Large**. Every other entry
        // on the ladder is Medium, which is why the shared chassis pins
        // Medium and this is the one override — a cloud of six hundred
        // bats fills more of a corridor than a knot of rats does.
        size: Size::Large,
        ..swarm_template(
            "Swarm of Bats",
            'ß',
            12,
            "2d10",
            5.,
            [5, 15, 10, 2, 12, 4],
            HashSet::from([SpecialSense::Blindsight(60)]),
            0.25,
            &SWARM_OF_BATS_BITES,
            true,
            HashSet::new(),
        )
    }
});

/// Swarm of Rats — CR ¼ Medium swarm of Tiny beasts. RAW: AC 10, 14 HP
/// (4d8-4), speed 30 ft, darkvision 30 ft, **no damage resistances**,
/// Bites 7 (2d6) piercing.
///
/// The swarm you can fight with a sword. RAW withholds the physical
/// resistances every other swarm gets — rats are big enough that a
/// blade swung through them connects — which makes this the whole
/// family's tutorial: it teaches the shape (one HP pool, thins at half,
/// can't be healed, can't be knocked down) without the lesson that
/// weapons don't work, and then the insect swarm two rungs up teaches
/// that.
///
/// It pays for the missing resistance with the family's worst AC (10)
/// and the fattest dice at its CR (2d6 to the bat swarm's 2d4).
pub static SWARM_OF_RATS_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    swarm_template(
        "Swarm of Rats",
        'ß',
        10,
        "4d8-4",
        30.,
        [9, 11, 9, 2, 10, 3],
        HashSet::from([SpecialSense::Darkvision(30)]),
        0.25,
        &SWARM_OF_RATS_BITES,
        false,
        HashSet::new(),
    )
});

/// Swarm of Insects — CR ½ Medium swarm of Tiny beasts. RAW: AC 11, 19
/// HP (3d8+6), speed 20 ft / climb 20 ft, blindsight 10 ft, resistant
/// to bludgeoning / piercing / slashing, Bites 10 (4d4) piercing.
///
/// The damage swarm, and the family's clearest statement of the
/// tactical problem: 4d4 a round on a 19-HP frame you can only really
/// hurt with fire. Its blindsight is 10 ft rather than the bat swarm's
/// 60 — it finds you by touch, not by echo, so unlike the bats it can
/// be hidden from, just not once it has arrived.
///
/// Slowest of the six at speed 20. That is the counterplay: a party
/// that keeps moving outruns it, and one that stands and swings at it
/// with steel does not.
pub static SWARM_OF_INSECTS_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    swarm_template(
        "Swarm of Insects",
        'ß',
        11,
        "3d8+6",
        20.,
        [3, 13, 14, 1, 7, 1],
        HashSet::from([SpecialSense::Blindsight(10)]),
        0.5,
        &SWARM_OF_INSECTS_BITES,
        true,
        HashSet::new(),
    )
});

/// Swarm of Piranhas — CR 1 Medium swarm of Tiny beasts. RAW: AC 13, 28
/// HP (8d8-8), speed 0 ft / swim 40 ft, darkvision 60 ft, resistant to
/// bludgeoning / piercing / slashing, **Blood Frenzy**, Bites 14 (4d6)
/// piercing.
///
/// The piranha swarm, and the only one with a trait beyond the shared
/// four. **Blood Frenzy** — "the swarm has advantage on melee attack
/// rolls against any creature that doesn't have all its hit points" —
/// rides the existing `BLOOD_FRENZY_TAG` that the Hunter Shark and
/// Sahuagin already read at `compute_attack_mode`, so the piranhas
/// inherit the whole gate for one tag.
///
/// The interaction with the swarm's own thinning is the thing to watch:
/// its 4d6 halves as it dies while its accuracy *improves* as its
/// target bleeds. A piranha swarm at 5 HP against a wounded fighter is
/// still landing every bite, for a third of what it used to.
///
/// Speed 40 for RAW's "0 ft, swim 40 ft" — the engine models one speed
/// magnitude, so the swim number is the one that matters; a swarm of
/// quippers on dry land is not an encounter.
/// Swarm of Ravens — CR ¼ Medium swarm of Tiny beasts. RAW: AC 12,
/// 11 HP (2d8+2), speed 10 ft / fly 50 ft, resistant to bludgeoning /
/// piercing / slashing, Beaks 5 (1d6 + 2) piercing.
///
/// The sixth swarm, and the one with eyes. Every other entry on the
/// ladder finds you by touch or by echo; the ravens have Perception +5
/// and ordinary sight, which on this board makes them the swarm a party
/// cannot hide from and the only one a dark room actually inconveniences.
///
/// Fly 50 makes it the fastest thing on the bench by twenty feet — the
/// bats manage 30 — and that is its whole tactical identity: a raven
/// swarm crosses the room in one turn, which is the difference between
/// a swarm the archers get two rounds on and a swarm they get none.
///
/// RAW's **Cacophony** (Recharge 6) — a DC 10 Wisdom save or the target
/// cannot take Reactions and has Disadvantage on ability checks and
/// attacks until the swarm's next turn — is not modeled. It is a
/// three-clause debuff on a recharge-6 pool, and the engine's
/// condition set has no single entry that means all three; splitting it
/// across two conditions on one save would be inventing a rule to hold
/// a rule.
pub static SWARM_OF_RAVENS_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    CreatureTemplate {
        // RAW speed line: Speed 10 ft., Fly 50 ft.
        fly_speed: 50.0,
        skills: HashSet::from([crate::engine::types::Skill::Perception]),
        ..swarm_template(
            "Swarm of Ravens",
            'ß',
            12,
            "2d8+2",
            10.,
            [6, 14, 12, 5, 12, 6],
            HashSet::new(),
            0.25,
            &SWARM_OF_RAVENS_BEAKS,
            true,
            HashSet::new(),
        )
    }
});

pub static SWARM_OF_PIRANHAS_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    swarm_template(
        "Swarm of Piranhas",
        'ß',
        13,
        "8d8-8",
        40.,
        [13, 16, 9, 1, 7, 2],
        HashSet::from([SpecialSense::Darkvision(60)]),
        1.0,
        &SWARM_OF_PIRANHAS_BITES,
        true,
        // The one swarm of the six that lives in the water — RAW's
        // "Speed 0 ft., swim 40 ft.", the only entry on the ladder with
        // no walking speed at all. The swim tag is what stops a pool
        // from charging it double to cross; the breathing tag is RAW's
        // "the swarm can breathe only underwater", which the engine
        // reads for its safe half alone (see `engine::breath`).
        HashSet::from([BLOOD_FRENZY_TAG, SWIM_SPEED_TAG, UNDERWATER_BREATHING_TAG]),
    )
});

/// Swarm of Venomous Snakes — CR 2 Medium swarm of Tiny beasts. RAW:
/// AC 14, 36 HP (8d8), speed 30 ft / swim 30 ft, blindsight 10 ft,
/// resistant to bludgeoning / piercing / slashing, Bites 7 (2d6)
/// piercing plus DC 10 CON save or 14 (4d6) poison, half on a success.
///
/// The top of the ladder, and the only swarm whose bite carries a
/// second damage type. On paper its piercing die is the rat swarm's;
/// the four CR rungs between them are all venom. A failed save roughly
/// triples the swing, and there is no attack roll standing between the
/// party and it — the poison rides a landed bite, so the only defence
/// is the CON save.
///
/// Also the toughest frame on the ladder (AC 14, 36 HP), which matters
/// more than it looks: the halving threshold is 18 HP, so a party
/// needs to land more raw damage on this swarm than on any other before
/// its bite starts shrinking — and against b/p/s resistance that is 36
/// points of sword.
pub static SWARM_OF_VENOMOUS_SNAKES_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    swarm_template(
        "Swarm of Venomous Snakes",
        'ß',
        14,
        "8d8",
        30.,
        [8, 18, 11, 1, 10, 3],
        HashSet::from([SpecialSense::Blindsight(10)]),
        2.0,
        &SWARM_OF_VENOMOUS_SNAKES_BITES,
        true,
        HashSet::new(),
    )
});

/// Every swarm statblock, for the sweeps that want to assert something
/// about all of them at once.
pub fn all_swarm_templates() -> [&'static CreatureTemplate; 6] {
    [
        &SWARM_OF_BATS_TEMPLATE,
        &SWARM_OF_RATS_TEMPLATE,
        &SWARM_OF_INSECTS_TEMPLATE,
        &SWARM_OF_PIRANHAS_TEMPLATE,
        &SWARM_OF_RAVENS_TEMPLATE,
        &SWARM_OF_VENOMOUS_SNAKES_TEMPLATE,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::{ActorInstance, HealOutcome};
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::Coordinate;

    fn make(template: &'static CreatureTemplate) -> ActorInstance {
        ActorInstance::from_creature_template(
            template,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    /// The four clauses that make a swarm a swarm hold on all six, so
    /// a sixth swarm added through the shared chassis inherits the
    /// sweep rather than needing its own copy of it.
    #[test]
    fn every_swarm_carries_the_shared_swarm_envelope() {
        for template in all_swarm_templates() {
            let a = make(template);
            let name = a.name().to_string();
            assert!(a.is_swarm(), "{name} should be flagged a swarm");
            // Size is the one line of the six stat blocks that is not
            // shared: SRD 5.2 prints the bat swarm as Large and the
            // other four as Medium. Asserted as a pair rather than
            // exempted, so a sixth swarm has to say which it is.
            let expected = if template.name == "Swarm of Bats" {
                Size::Large
            } else {
                Size::Medium
            };
            assert_eq!(a.size(), expected, "{name} has the wrong size");
            assert_eq!(a.creature_type(), CreatureType::Beast, "{name}");
            for condition in swarm_condition_immunities() {
                assert!(
                    a.is_immune_to_condition(condition),
                    "{name} should be immune to {condition:?}"
                );
            }
        }
    }

    /// "The swarm can't regain hit points or gain temporary hit
    /// points." Both halves, on a swarm that has room for both.
    #[test]
    fn a_swarm_refuses_healing_and_temp_hp() {
        let mut a = make(&SWARM_OF_VENOMOUS_SNAKES_TEMPLATE);
        a.take_damage(10);
        let wounded = a.hitpoints();
        assert!(wounded > 0, "the test needs a swarm that is hurt, not dead");
        assert_eq!(a.heal(5), HealOutcome::NoOp);
        assert_eq!(a.hitpoints(), wounded);
        assert_eq!(a.gain_temp_hp(5), 0);
        assert_eq!(a.temp_hp(), 0);
    }

    /// The bite halves at exactly half hit points and not one point
    /// above, and only for swarms.
    #[test]
    fn the_bite_thins_at_half_hit_points() {
        let mut a = make(&SWARM_OF_INSECTS_TEMPLATE);
        let max = a.max_hitpoints();
        assert!(!a.is_thinned_swarm(), "a full swarm bites in full");
        // One point above half is still the full swing. `max / 2 + 1`
        // is the smallest HP total that clears `hp * 2 <= max` at both
        // odd and even maxima.
        a.take_damage(max - (max / 2 + 1));
        assert!(!a.is_thinned_swarm(), "{} of {} is above half", a.hitpoints(), max);
        a.take_damage(1);
        assert!(a.is_thinned_swarm(), "{} of {} is at or below half", a.hitpoints(), max);
    }

    /// A wounded non-swarm hits exactly as hard as a fresh one — the
    /// thinning rule is the swarm's own, not a general bloodied
    /// threshold.
    #[test]
    fn a_wounded_ordinary_creature_is_not_thinned() {
        use crate::actors::creatures::ogres::OGRE_TEMPLATE;

        let mut a = make(&OGRE_TEMPLATE);
        a.take_damage(a.max_hitpoints() - 1);
        assert!(!a.is_swarm());
        assert!(!a.is_thinned_swarm());
    }

    /// The rat swarm is the one you can hit with a sword, and the
    /// insect swarm is the one you can't. RAW's single most
    /// tactically-relevant split inside the family.
    #[test]
    fn only_the_rat_swarm_lacks_physical_resistance() {
        for damage_type in [
            DamageType::Bludgeoning,
            DamageType::Piercing,
            DamageType::Slashing,
        ] {
            assert_eq!(
                make(&SWARM_OF_RATS_TEMPLATE).damage_modifier(damage_type),
                None,
                "rats are big enough to hit"
            );
            assert_eq!(
                make(&SWARM_OF_INSECTS_TEMPLATE).damage_modifier(damage_type),
                Some(DamageModifier::Resistance),
                "a blade passes through insects"
            );
        }
    }

    /// The piranha swarm is the only one with a trait beyond the shared
    /// four, and it reuses the existing shark / sahuagin tag rather
    /// than a swarm-specific one.
    #[test]
    fn only_the_piranha_swarm_frenzies() {
        for template in all_swarm_templates() {
            // Compared against the *template* name: an instance's
            // `name()` carries a disambiguating suffix ("Swarm of
            // Piranhas 0") that would make an equality check here
            // silently always-false.
            let expected = template.name == "Swarm of Piranhas";
            assert_eq!(
                make(template).has_passive_feature(BLOOD_FRENZY_TAG),
                expected,
                "{}",
                template.name
            );
        }
    }
}
