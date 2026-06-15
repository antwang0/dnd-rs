use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    CENTAUR_HOOVES, CENTAUR_MULTI, CENTAUR_PIKE, LONGBOW,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Centaur — CR 2 large monstrosity. Sturdy hybrid kit: a reach-2 pike
/// for opening jabs, a 2d6 hoof kick for the second swing, and a longbow
/// for ranged fallback when out of melee. Multi pairs pike + hooves so a
/// committed melee turn brings 1d10 + 2d6 down on a single target.
/// Fast 50ft walking speed reflects the equine half — kites comfortably
/// between bow shots.
pub static CENTAUR_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&CENTAUR_PIKE);
    actions.push(&CENTAUR_HOOVES);
    actions.push(&LONGBOW);
    actions.push(&*CENTAUR_MULTI);
    CreatureTemplate {
        name: "Centaur",
        // 'C' was free in the medium / large monstrosity slot — capital
        // because Large.
        glyph: 'C',
        ac: 12,
        hitpoints: "5d10+10".parse().unwrap(),
        speed: 50.,
        strength: 18,
        intelligence: 9,
        dexterity: 14,
        wisdom: 13,
        constitution: 14,
        charisma: 11,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        languages: HashSet::from([Language::Elvish, Language::Sylvan]),
        cr: 2.0,
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
        has_deflect_missiles: false,
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
