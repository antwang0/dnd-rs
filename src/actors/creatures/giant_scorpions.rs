use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{GIANT_SCORPION_CLAW, GIANT_SCORPION_STING};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Giant Scorpion — CR 3 beast. Massive arachnid with two claws and a
/// venomous stinger. Claw deals 1d8+2 bludgeoning + grapple on hit.
/// Sting deals 1d10+2 piercing + 4d10 poison (CON save DC 12 for half).
/// AC 15, ~52 HP (7d10+14). Blindsight 60ft. Multiattack: two claws +
/// one sting per turn.
pub static GIANT_SCORPION_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*GIANT_SCORPION_CLAW);
    actions.push(&*GIANT_SCORPION_STING);
    CreatureTemplate {
        name: "Giant Scorpion",
        glyph: '†',
        ac: 15,
        hitpoints: "7d10+14".parse().unwrap(),
        speed: 40.,
        strength: 15,
        intelligence: 1,
        dexterity: 13,
        wisdom: 9,
        constitution: 15,
        charisma: 3,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Blindsight(60)]),
        languages: HashSet::new(),
        cr: 3.0,
        size: Size::Large,
        creature_type: CreatureType::Beast,
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
        has_aura_of_protection: false,
        has_aura_of_courage: false,
    }
});
