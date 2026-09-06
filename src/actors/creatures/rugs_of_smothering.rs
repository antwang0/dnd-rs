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
        name: "Rug of Smothering",
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
        damage_modifiers: damage_modifiers_from([(DamageType::Poison, DamageModifier::Immunity)]),
        condition_immunities: HashSet::from([
            // SRD 5.2 "Immunities Poison, Psychic; Charmed, Deafened,
            // Exhaustion, Frightened, Paralyzed, Petrified, Poisoned".
            Condition::Exhausted,
            Condition::Blinded,
            Condition::Charmed,
            Condition::Deafened,
            Condition::Frightened,
            Condition::Paralyzed,
            Condition::Petrified,
            Condition::Poisoned,
            Condition::Prone,
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

    /// A rug is already on the floor. Prone immunity is the one
    /// condition on the construct list that is funny rather than
    /// obvious, and it is the one a copy-pasted envelope would drop.
    #[test]
    fn you_cannot_knock_over_a_carpet() {
        let a = make();
        assert!(a.effectively_immune_to_condition(Condition::Prone));
        assert!(a.effectively_immune_to_condition(Condition::Blinded));
    }
}
