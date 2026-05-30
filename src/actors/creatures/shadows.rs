use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::LifeDrain;
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Size, SpecialSense};
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
        hitpoints: "3d8+3".parse().unwrap(),
        speed: 40.,
        strength: 6,
        intelligence: 6,
        dexterity: 14,
        wisdom: 10,
        constitution: 13,
        charisma: 8,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::new(),
        cr: 0.5,
        size: Size::Medium,
        creature_type: CreatureType::Undead,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
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
        proficient_saves: HashSet::new(),
        condition_immunities: HashSet::from([
            Condition::Frightened,
            Condition::Grappled,
            Condition::Paralyzed,
            Condition::Petrified,
            Condition::Poisoned,
            Condition::Prone,
            Condition::Restrained,
        ]),
        features: HashSet::new(),
        regen_per_round: 0,
        regen_suppressors: HashSet::new(),
        legendary_resistances: 0,
        has_evasion: false,
        has_uncanny_dodge: false,
        has_displacement: false,
        has_danger_sense: false,
        has_pack_tactics: false,
        has_magic_resistance: false,
        recharge_abilities: Vec::new(),
        legendary_actions_per_round: 0,
        has_extra_attack: false,
        brutal_critical_dice: 0,
        crit_threshold: 20,
        has_lucky: false,
        has_aura_of_protection: false,
        has_aura_of_courage: false,
        has_savage_attacks: false,
        has_dwarven_resilience: false,
        sorcery_points: 0,
    }
});
