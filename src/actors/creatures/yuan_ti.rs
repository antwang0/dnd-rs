use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{SCIMITAR, YUAN_TI_MALISON_BITE, YUAN_TI_MALISON_MULTI};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Yuan-Ti Malison — CR 3 medium fiend hybrid. Snake-headed humanoid
/// shock troops with a signature scimitar + venomous-bite multi: the
/// scimitar lands 1d6+STR slashing while the bite probes for a CON
/// save vs Poisoned (10-round lockout). RAW magic resistance (advantage
/// on save vs spells) marks it as a tougher-than-CR-implies caster
/// counter — slots cleanly between the cult fanatic (CR 2 caster) and
/// the bone devil (CR 9 outsider) in the mid-tier fiend pool.
///
/// Templates: AC 12, 66 HP (12d8+12), STR 16 (+3), DEX 14, CON 13,
/// INT 14, WIS 12, CHA 14. Languages: Abyssal, Common, Draconic.
/// Darkvision 60 ft (RAW: 120ft; we cap at 60 to match the standard
/// fiend darkvision tier in this engine).
pub static YUAN_TI_MALISON_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&SCIMITAR);
    actions.push(&*YUAN_TI_MALISON_BITE);
    actions.push(&*YUAN_TI_MALISON_MULTI);
    CreatureTemplate {
        name: "Yuan-Ti Malison",
        // 'Y' for the fiend hybrid — capital because medium-sized but
        // visually distinctive ('y' lowercase reads as a yeti's pup).
        glyph: 'Y',
        ac: 12,
        hitpoints: "12d8+12".parse().unwrap(),
        speed: 30.,
        strength: 16,
        intelligence: 14,
        dexterity: 14,
        wisdom: 12,
        constitution: 13,
        charisma: 14,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Abyssal, Language::Common, Language::Draconic]),
        cr: 3.0,
        size: Size::Medium,
        creature_type: CreatureType::Fiend,
        actions,
        // Yuan-ti malison is poison-immune (snake-blooded) — folds into
        // the same damage-pipeline lane as bone devil / pit fiend.
        damage_modifiers: HashMap::from([(DamageType::Poison, DamageModifier::Immunity)]),
        // RAW: immune to Poisoned condition (their bodies metabolize
        // poison as fuel).
        condition_immunities: HashSet::from([Condition::Poisoned]),
        // Magic Resistance: advantage on saving throws against spells.
        // Same template flag as the green hag / lich uses.
        has_magic_resistance: true,
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
    fn yuan_ti_malison_is_poison_immune() {
        let a = ActorInstance::from_creature_template(
            &YUAN_TI_MALISON_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert!(a.is_immune_to(DamageType::Poison));
        assert!(a.is_immune_to_condition(Condition::Poisoned));
    }

    #[test]
    fn yuan_ti_malison_has_magic_resistance() {
        let a = ActorInstance::from_creature_template(
            &YUAN_TI_MALISON_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert!(a.has_magic_resistance());
    }
}
