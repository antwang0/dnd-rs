use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{GOBLIN_BOSS_MULTI, SHORTBOW};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Goblin Boss — 5e MM CR 1, the tougher cousin of the standard goblin.
/// Multiattack: two scimitar swings per Action, a real upgrade from the
/// regular goblin's single swing. Higher AC (chain shirt + shield) and
/// more HP make this the closest thing the codebase has to a "miniboss"
/// — hard enough to break a low-CR encounter open without a full party.
pub static GOBLIN_BOSS_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*GOBLIN_BOSS_MULTI);
    actions.push(&SHORTBOW);
    CreatureTemplate {
        name: "Goblin Boss",
        glyph: 'B',
        ac: 17, // chain shirt + shield
        hitpoints: "6d6+6".parse().unwrap(),
        speed: 30.,
        strength: 10,
        intelligence: 10,
        dexterity: 14,
        wisdom: 8,
        constitution: 10,
        charisma: 10,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common, Language::Goblin]),
        cr: 1.0,
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
        has_gnome_cunning: false,
        draconic_ancestry: None,
        sorcery_points: 0,
    }
});
