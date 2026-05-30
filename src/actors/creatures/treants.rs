use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{TREANT_MULTIATTACK, TREANT_SLAM};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Language, Size};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Treant — CR 9 plant. Slow, towering HP wall. Two-slam multiattack at
/// reach 2 (10 ft); resistant to bludgeoning and piercing (its bark
/// shrugs off arrows and clubs) but vulnerable to fire — fire damage
/// blasts the plant's bark into kindling. Plants ignore Poisoned /
/// Charmed / Frightened (close to RAW's "creature type: plant"
/// immunity envelope but not identical; we collapse to the
/// most-relevant tags).
pub static TREANT_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&TREANT_SLAM);
    actions.push(&*TREANT_MULTIATTACK);
    CreatureTemplate {
        name: "Treant",
        // 't' (lowercase) is free. Uppercase 'T' is the troll.
        glyph: 't',
        ac: 16,
        // 12d12+60 = 138 average per MM — CR-9 sized HP wall.
        hitpoints: "12d12+60".parse().unwrap(),
        speed: 30.,
        strength: 23,
        intelligence: 12,
        dexterity: 8,
        wisdom: 16,
        constitution: 21,
        charisma: 12,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        languages: HashSet::from([Language::Common, Language::Sylvan, Language::Druidic]),
        cr: 9.0,
        size: Size::Huge,
        creature_type: CreatureType::Plant,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        // 5e plant: bark resists physical clubs / arrows but burns
        // easily. Vulnerability to fire is the standard counter — a
        // single Fireball strips 2x damage off the HP wall.
        damage_modifiers: HashMap::from([
            (DamageType::Bludgeoning, DamageModifier::Resistance),
            (DamageType::Piercing, DamageModifier::Resistance),
            (DamageType::Fire, DamageModifier::Vulnerability),
        ]),
        proficient_saves: HashSet::new(),
        // Plant-creature condition envelope: ignores poisoned / charmed
        // / frightened. Doesn't sleep either (Sleep already breaks on
        // HP totals this high but the explicit tag is cleaner).
        condition_immunities: HashSet::from([
            Condition::Poisoned,
            Condition::Charmed,
            Condition::Frightened,
            Condition::Asleep,
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
