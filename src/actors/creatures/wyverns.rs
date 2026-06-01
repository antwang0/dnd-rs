use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{WYVERN_BITE, WYVERN_STINGER};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Wyvern — CR 6 dragon. Large-footprint flying predator with two
/// action lanes:
/// - Bite: 2d6 + STR piercing melee (reach 2 ≈ 10ft).
/// - Stinger: 2d6 + STR piercing melee plus a brutal CON-save poison
///   rider (7d6 poison on fail, half on save) — the wyvern's signature
///   finisher.
///
/// No language slot per MM (wyverns are non-sentient predators); we
/// leave languages empty rather than tag a placeholder. No condition
/// immunities — wyverns are mortal, just very angry.
pub static WYVERN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&WYVERN_BITE);
    actions.push(&*WYVERN_STINGER);
    CreatureTemplate {
        name: "Wyvern",
        // 'y' (lowercase) is free; uppercase 'Y' is Yeti.
        glyph: 'y',
        ac: 13,
        // 13d10+39 = ~110 average per MM (CR 6).
        hitpoints: "13d10+39".parse().unwrap(),
        speed: 40.,
        strength: 19,
        intelligence: 5,
        dexterity: 10,
        wisdom: 12,
        constitution: 16,
        charisma: 6,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::new(),
        cr: 6.0,
        size: Size::Large,
        creature_type: CreatureType::Monstrosity,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        damage_modifiers: HashMap::new(),
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
        has_aura_of_protection: false,
        has_aura_of_courage: false,
        has_savage_attacks: false,
        has_dwarven_resilience: false,
        has_gnome_cunning: false,
        draconic_ancestry: None,
        sorcery_points: 0,
    }
});
