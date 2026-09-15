use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::STRENGTH_DRAIN;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::lighting::SunlightFrailty;
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Size, Skill, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Shadow — incorporeal undead (CR ½). Skulks in darkness and drains
/// the strength out of the living. Resistant to acid, cold, fire,
/// lightning and thunder; immune to necrotic and poison; vulnerable to
/// radiant. Condition immunities match the incorporeal-undead envelope.
///
/// **Strength Drain** is the stat block, and the shadow had been
/// swinging the *wraith's* Life Drain instead — a CR-½ creature
/// borrowing a CR-5 one's dice, because the engine had a lane for a
/// drained hit point maximum and none for a drained ability score. See
/// `monster_attacks::STRENGTH_DRAIN` for what the difference is worth,
/// and `ActorInstance::ability_drain` for the lane.
///
/// **Amorphous**, not Incorporeal Movement: the shadow's own trait is
/// *"can move through a space as narrow as 1 inch"*, which is a crack
/// rather than a wall and has nothing to say on a grid whose smallest
/// unit is two and a half feet. See
/// `creatures::incorporeal_templates`, which names the absence.
pub static SHADOW_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*STRENGTH_DRAIN);
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
