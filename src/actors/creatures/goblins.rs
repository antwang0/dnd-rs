use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{SCIMITAR, SHORTBOW};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Sneaky melee skirmisher that doubles up its turn with a bonus-action
/// shortbow shot. Action: scimitar (close in and slash). Bonus: shortbow
/// (extra ranged ping). The action-economy split is the headline — most
/// creatures don't have a bonus-action attack option.
pub static GOBLIN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&SCIMITAR);
    actions.push(&SHORTBOW);
    CreatureTemplate {
        name: "Goblin",
        glyph: 'G',
        ac: 15,
        hitpoints: "2d6".parse().unwrap(),
        speed: 30.,
        strength: 8,
        intelligence: 10,
        dexterity: 14,
        wisdom: 8,
        constitution: 10,
        charisma: 8,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common, Language::Goblin]),
        cr: 0.25,
        size: Size::Small,
        creature_type: CreatureType::Humanoid,
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
        has_savage_attacks: false,
        has_dwarven_resilience: false,
        sorcery_points: 0,
    }
});
