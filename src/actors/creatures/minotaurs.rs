use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{GREATAXE, MINOTAUR_GORE};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{Language, Size};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Minotaur — CR 3 monstrosity. Mid-tier melee bruiser with two action
/// options: a big greataxe swing (1d12+STR slashing) or a gore charge
/// (2d8+STR piercing) — both single-target heavy hitters tuned for the
/// AI to pick between depending on what's in reach. AC 14 with 76
/// average HP makes them notably tankier than the bandit-tier mooks.
pub static MINOTAUR_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*GREATAXE);
    actions.push(&*MINOTAUR_GORE);
    CreatureTemplate {
        name: "Minotaur",
        // 'N' for miNotaur — distinct from existing glyphs.
        glyph: 'N',
        ac: 14,
        // 9d10+27 = 76 average per MM.
        hitpoints: "9d10+27".parse().unwrap(),
        speed: 40.,
        strength: 18,
        intelligence: 6,
        dexterity: 11,
        wisdom: 16,
        constitution: 16,
        charisma: 9,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        languages: HashSet::from([Language::Common]),
        cr: 3.0,
        size: Size::Large,
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
