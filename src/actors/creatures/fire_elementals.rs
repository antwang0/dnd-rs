use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::FIRE_ELEMENTAL_TOUCH;
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{DamageModifier, DamageType, Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Fire Elemental — CR 5 elemental. Walking inferno: fire-touch melee
/// for 2d6 + ignite (Burning DOT). Immune to fire and poison, resistant
/// to non-magical physical damage (we collapse to "physical resistance"
/// since the engine doesn't yet track magical-weapon distinctions).
/// Elemental condition envelope: ignores almost every mind / body
/// control condition (Charmed, Frightened, Paralyzed, Petrified,
/// Poisoned, Asleep, Prone) — close to the 5e MM line.
pub static FIRE_ELEMENTAL_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*FIRE_ELEMENTAL_TOUCH);
    CreatureTemplate {
        name: "Fire Elemental",
        // 'E' (uppercase) — free; uppercase 'F' is the fighter.
        glyph: 'E',
        ac: 13,
        // 10d10+20 = 75 average per MM.
        hitpoints: "10d10+20".parse().unwrap(),
        speed: 50.,
        strength: 10,
        intelligence: 6,
        dexterity: 17,
        wisdom: 10,
        constitution: 16,
        charisma: 7,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Primordial]),
        cr: 5.0,
        size: Size::Large,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        damage_modifiers: HashMap::from([
            (DamageType::Fire, DamageModifier::Immunity),
            (DamageType::Poison, DamageModifier::Immunity),
            // RAW: "resistance to bludgeoning, piercing, slashing from
            // non-magical attacks." We don't track magical-weapon
            // distinction so we apply the broader resistance.
            (DamageType::Bludgeoning, DamageModifier::Resistance),
            (DamageType::Piercing, DamageModifier::Resistance),
            (DamageType::Slashing, DamageModifier::Resistance),
        ]),
        proficient_saves: HashSet::new(),
        condition_immunities: HashSet::from([
            Condition::Charmed,
            Condition::Frightened,
            Condition::Paralyzed,
            Condition::Petrified,
            Condition::Poisoned,
            Condition::Asleep,
            Condition::Prone,
            Condition::Grappled,
            Condition::Restrained,
        ]),
        features: HashSet::new(),
        regen_per_round: 0,
        regen_suppressors: HashSet::new(),
        legendary_resistances: 0,
    }
});
