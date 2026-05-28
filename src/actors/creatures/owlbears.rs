use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::OWLBEAR_MULTIATTACK;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Owlbear — CR 3 monstrosity. Brute-stat bruiser whose only flair is
/// the multiattack: every Action swings beak (1d10 piercing) AND claws
/// (2d8 slashing). Heavy STR-based damage, no rider effects, decent HP
/// pool. Filling a "pure damage threat at higher CR" niche the troll
/// shares but without regeneration.
pub static OWLBEAR_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*OWLBEAR_MULTIATTACK);
    CreatureTemplate {
        name: "Owlbear",
        // 'b' (lowercase b) — distinct from 'B' (Bugbear) and 'b' is
        // currently free.
        glyph: 'b',
        ac: 13,
        hitpoints: "7d10+21".parse().unwrap(),
        speed: 40.,
        strength: 20,
        intelligence: 3,
        dexterity: 12,
        wisdom: 12,
        constitution: 17,
        charisma: 7,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::new(),
        cr: 3.0,
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
    }
});
