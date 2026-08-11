use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::HOMUNCULUS_BITE;
use crate::actors::actor_template::{CreatureTemplate, damage_modifiers_from};
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Homunculus — CR 0 tiny construct. A wizard's familiar made from
/// their own blood, five hit points, and a bite that carries more
/// threat than the whole rest of the creature.
///
/// Action lane: **homunculus bite**, 1d4 piercing with a DC 10 CON save
/// for 2d4 poison and the Poisoned condition. RAW's bite deals *one
/// point*; the venom is the attack, and the fail-by-5 unconsciousness
/// clause is why a CR 0 construct is worth putting on a board. The
/// engine has no fail-by-N lane, so the poison damage and the condition
/// stand in for it.
///
/// The construct envelope is the usual one and is most of what keeps
/// this alive long enough to bite twice: poison immunity (which is
/// funny on a creature whose only attack is poison, and RAW), plus
/// immunity to charm, exhaustion, and being poisoned or paralyzed.
///
/// Stat shape per the SRD: AC 13 (natural armor), 5 HP (2d4), STR 4 /
/// DEX 15 / CON 11 / INT 10 / WIS 10 / CHA 7. Speed 20 walking, 40
/// flying — the engine has one speed and takes the wings. Darkvision
/// 60. CR 0.
pub static HOMUNCULUS_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&HOMUNCULUS_BITE);
    CreatureTemplate {
        name: "Homunculus",
        // 'x' (lowercase) — free, and deliberately not 'h' (Hyena) or
        // 'H' (the hobgoblin / harpy band): a homunculus should not read
        // as either at a glance.
        glyph: 'x',
        ac: 13,
        // 2d4 ≈ 5 average per the SRD (CR 0).
        hitpoints: "2d4".parse().unwrap(),
        // RAW speed line: Speed 20 ft., fly 40 ft.
        speed: 20.0,
        fly_speed: 40.0,
        strength: 4,
        dexterity: 15,
        constitution: 11,
        intelligence: 10,
        wisdom: 10,
        charisma: 7,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        cr: 0.0,
        size: Size::Tiny,
        creature_type: CreatureType::Construct,
        actions,
        damage_modifiers: damage_modifiers_from([(DamageType::Poison, DamageModifier::Immunity)]),
        condition_immunities: HashSet::from([
            Condition::Charmed,
            Condition::Exhausted,
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

    #[test]
    fn homunculus_template_shape() {
        let a = ActorInstance::from_creature_template(
            &HOMUNCULUS_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 0.0);
        assert_eq!(a.size(), Size::Tiny);
        assert_eq!(a.creature_type(), CreatureType::Construct);
        assert!(a.find_action("homunculus bite").is_some());
        // Poison-immune, and its only attack is poison. RAW, and worth
        // pinning because it looks like a mistake.
        assert_eq!(
            a.damage_modifier(DamageType::Poison),
            Some(DamageModifier::Immunity)
        );
    }
}
