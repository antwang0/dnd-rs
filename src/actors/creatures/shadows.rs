use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::LifeDrain;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::lighting::SunlightFrailty;
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Size, Skill, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

static SHADOW_DRAIN: LazyLock<LifeDrain> = LazyLock::new(|| LifeDrain {});

/// Shadow — incorporeal undead (CR 1/2, MM p.269). Skulks in darkness
/// and drains the life force of the living. Resistant to acid, cold,
/// fire, lightning, and thunder; immune to necrotic and poison. Vulnerable
/// to radiant. Condition immunities match typical undead incorporeals.
pub static SHADOW_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*SHADOW_DRAIN);
    CreatureTemplate {
        name: "Shadow",
        glyph: 's',
        ac: 12,
        hitpoints: "5d8+5".parse().unwrap(),
        speed: 40.,
        strength: 6,
        dexterity: 14,
        constitution: 13,
        intelligence: 6,
        wisdom: 10,
        charisma: 8,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        cr: 0.5,
        size: Size::Medium,
        creature_type: CreatureType::Undead,
        actions,
        damage_modifiers: HashMap::from([
            (DamageType::Acid, DamageModifier::Resistance),
            (DamageType::Cold, DamageModifier::Resistance),
            (DamageType::Fire, DamageModifier::Resistance),
            (DamageType::Lightning, DamageModifier::Resistance),
            (DamageType::Thunder, DamageModifier::Resistance),
            (DamageType::Necrotic, DamageModifier::Immunity),
            (DamageType::Poison, DamageModifier::Immunity),
            (DamageType::Radiant, DamageModifier::Vulnerability),
        ]),
        condition_immunities: HashSet::from([
            // SRD 5.2 "Immunities Necrotic, Poison; Exhaustion, Frightened,
            // Grappled, Paralyzed, Petrified, Poisoned, Prone,
            // Restrained, Unconscious".
            Condition::Exhausted,
            Condition::Frightened,
            Condition::Grappled,
            Condition::Paralyzed,
            Condition::Petrified,
            Condition::Poisoned,
            Condition::Prone,
            Condition::Restrained,
            Condition::Unconscious,
        ]),
        skills: HashSet::from([Skill::Stealth]),
        // 5e Shadow **Sunlight Weakness**: "while in sunlight, the
        // shadow has disadvantage on attack rolls, ability checks, and
        // saving throws." One clause more than the kobold's
        // Sensitivity, and it is the clause that matters — a shadow
        // caught in the open fails the saves it would otherwise make.
        sunlight_frailty: Some(SunlightFrailty::Weakness),
        ..CreatureTemplate::defaults()
    }
});
