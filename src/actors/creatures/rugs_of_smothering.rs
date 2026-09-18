use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::RUG_OF_SMOTHERING_SMOTHER;
use crate::actors::actor_template::{CreatureTemplate, damage_modifiers_from};
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Rug of Smothering — CR 2 large construct. An animated carpet that
/// waits on a floor until somebody stands on it.
///
/// Action lane: **rug of smothering smother**, 2d6+STR bludgeoning that
/// Restrains on hit. One attack, no save, and a creature under it is
/// not getting out this turn.
///
/// **Damage Transfer** is the clause that makes the stat block
/// interesting and the one not carried: RAW, while the rug is smothering
/// a creature, it takes only half the damage dealt to it and the
/// creature underneath takes the other half — so hitting the rug hits
/// your own party member. It needs a per-victim damage-splitting link,
/// which the engine has for mounts and for nothing else, and wiring it
/// through a second relationship would be a rule only one creature
/// obeys. Named here rather than dropped silently, because it is the
/// first thing a reader will look for.
///
/// **Antimagic Susceptibility** is likewise absent — the engine's
/// Antimagic Field suppresses spells at the casting gate rather than
/// incapacitating constructs standing in it.
///
/// What is kept is what a party meets: a thirty-three-hit-point rug
/// with the construct's whole condition envelope, which cannot be
/// blinded (no eyes), charmed or frightened (no mind), or knocked prone
/// (it is already flat).
///
/// Stat shape per the SRD: AC 12, 27 HP (5d10), STR 17 / DEX 14 / CON
/// 10 / INT 1 / WIS 3 / CHA 1. Speed 10 — it does not chase. Blindsight
/// 60 (blind beyond). CR 2.
pub static RUG_OF_SMOTHERING_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&RUG_OF_SMOTHERING_SMOTHER);
    CreatureTemplate {
        // SRD 5.2 calls this stat block **Animated Rug of Smothering**; the engine
        // carried the 2014 heading ("Rug of Smothering") until the sweep that
        // compares the two had to keep a translation table to do
        // its job. The file and the static keep their old spelling,
        // because that is the word this codebase files the creature
        // under and moving it buys nothing a reader wants; the name
        // a player sees is the book's.
        name: "Animated Rug of Smothering",
        // 'r' (lowercase) — free; 'R' is the Roper / Rogue / Remorhaz
        // band and this is not one of those.
        glyph: 'r',
        ac: 12,
        // 5d10 ≈ 27 average per the SRD (CR 2).
        hitpoints: "5d10".parse().unwrap(),
        // RAW speed line: Speed 10 ft., and nothing else. SRD 5.2 gives
        // the rug no fly speed; the older printing's "fly 10 ft.
        // (hover)" had come across with the template, and the hover
        // annotation with it. A rug that hovers over a floor is a rug
        // that never lies still on one, which is the opposite of what
        // it is for.
        speed: 10.0,
        strength: 17,
        dexterity: 14,
        constitution: 10,
        intelligence: 1,
        wisdom: 3,
        charisma: 1,
        senses: HashSet::from([SpecialSense::Blindsight(60)]),
        cr: 2.0,
        size: Size::Large,
        creature_type: CreatureType::Construct,
        actions,
        // SRD 5.2: *"Immunities Poison, Psychic"* — the universal
        // construct pair, and the Psychic half was missing.
        damage_modifiers: damage_modifiers_from([
            (DamageType::Poison, DamageModifier::Immunity),
            (DamageType::Psychic, DamageModifier::Immunity),
        ]),
        // Two rows came off this list and both were the previous
        // printing's: `Blinded`, which every animated object used to
        // carry, and `Prone`, which is the one that mattered — a rug
        // lying flat is the *shape* Prone describes, and SRD 5.2 does
        // not exempt it. A rug that can be knocked over is a rug a
        // shove answers.
        condition_immunities: HashSet::from([
            // SRD 5.2 "Immunities Poison, Psychic; Charmed, Deafened,
            // Exhaustion, Frightened, Paralyzed, Petrified, Poisoned".
            Condition::Exhausted,
            Condition::Charmed,
            Condition::Deafened,
            Condition::Frightened,
            Condition::Paralyzed,
            Condition::Petrified,
            Condition::Poisoned,
        ]),
        ..CreatureTemplate::defaults()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::Coordinate;

    fn make() -> ActorInstance {
        ActorInstance::from_creature_template(
            &RUG_OF_SMOTHERING_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn rug_of_smothering_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 2.0);
        assert_eq!(a.size(), Size::Large);
        assert_eq!(a.creature_type(), CreatureType::Construct);
        assert!(a.find_action("rug of smothering smother").is_some());
    }

    /// A rug *can* be knocked over, and that is the 2024 page rather
    /// than an oversight.
    ///
    /// This test used to assert the opposite, under a comment calling
    /// Prone immunity *"the one condition on the construct list that is
    /// funny rather than obvious"*. It is funny, and SRD 5.2 does not
    /// print it: the Immunities line is Charmed, Deafened, Exhaustion,
    /// Frightened, Paralyzed, Petrified, Poisoned, and Blinded and
    /// Prone are both the previous printing's. A rug that a shove
    /// answers is a rug the party has one more thing to do about.
    ///
    /// Pinned in the negative rather than deleted, because the funny
    /// reading is the one somebody will reach for again.
    #[test]
    fn a_carpet_is_flat_and_can_still_be_knocked_over() {
        let a = make();
        assert!(!a.effectively_immune_to_condition(Condition::Prone));
        assert!(!a.effectively_immune_to_condition(Condition::Blinded));
        // …and the seven the page does print are all there.
        for immune in [
            Condition::Charmed,
            Condition::Deafened,
            Condition::Exhausted,
            Condition::Frightened,
            Condition::Paralyzed,
            Condition::Petrified,
            Condition::Poisoned,
        ] {
            assert!(
                a.effectively_immune_to_condition(immune),
                "SRD 5.2 prints {immune:?} on the rug's Immunities line"
            );
        }
    }
}
