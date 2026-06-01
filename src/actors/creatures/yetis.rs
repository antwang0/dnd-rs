use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{CHILLING_GAZE, YETI_MULTI};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Yeti — CR 3 monstrosity. A frozen-mountains predator with two
/// signature plays: a 2x claw multiattack (slashing + cold rider on
/// each hit) for adjacent enemies, or a chilling gaze (CON save vs
/// 3d6 cold + paralysis) for ranged crowd-control. Immune to cold,
/// vulnerable to fire — the natural counter is a fire spell.
pub static YETI_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*YETI_MULTI);
    actions.push(&*CHILLING_GAZE);
    CreatureTemplate {
        name: "Yeti",
        // 'Y' — currently free.
        glyph: 'Y',
        ac: 12,
        // 7d10+14 = 51 average per MM.
        hitpoints: "7d10+14".parse().unwrap(),
        speed: 40.,
        strength: 18,
        intelligence: 8,
        dexterity: 13,
        wisdom: 12,
        constitution: 15,
        charisma: 7,
        skills: HashSet::new(),
        items: Vec::new(),
        // Yetis have keen smell (60 ft) — closest engine analogue is
        // Blindsight, which lets them locate within a short radius.
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::new(),
        cr: 3.0,
        size: Size::Large,
        creature_type: CreatureType::Monstrosity,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        damage_modifiers: HashMap::from([
            (DamageType::Cold, DamageModifier::Immunity),
            (DamageType::Fire, DamageModifier::Vulnerability),
        ]),
        // 5e MM Yeti has no save proficiencies.
        proficient_saves: HashSet::new(),
        condition_immunities: HashSet::new(),
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
        has_brave: false,
        has_fey_ancestry: false,
        has_aura_of_protection: false,
        has_aura_of_courage: false,
        has_savage_attacks: false,
        has_dwarven_resilience: false,
        has_gnome_cunning: false,
        draconic_ancestry: None,
        sorcery_points: 0,
    }
});
