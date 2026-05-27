use crate::actions::class_attacks::ROGUE_SHORTSWORD;
use crate::actions::class_features::{CUNNING_DASH, CUNNING_DISENGAGE, CUNNING_HIDE};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, Language, Size};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Rogue PC template. Light armor (AC 14: leather + DEX), modest HP,
/// DEX-primary. The headline mechanic is **Sneak Attack** — the
/// shortsword (finesse, DEX-based 1d6) deals an extra 1d6 once per turn
/// when the rogue has advantage OR an ally is adjacent to the target.
/// `rolls_death_saves: true` (PC) so it enters the dying state at 0 HP.
pub static ROGUE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*ROGUE_SHORTSWORD);
    actions.push(&*CUNNING_DASH);
    actions.push(&*CUNNING_DISENGAGE);
    actions.push(&*CUNNING_HIDE);
    CreatureTemplate {
        name: "Rogue",
        glyph: 'R',
        ac: 14,
        hitpoints: "3d8+3".parse().unwrap(),
        speed: 30.,
        strength: 10,
        intelligence: 12,
        dexterity: 16, // primary
        wisdom: 12,
        constitution: 12,
        charisma: 10,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        languages: HashSet::from([Language::Common, Language::ThievesCant]),
        cr: 1.0,
        size: Size::Medium,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: true,
        damage_modifiers: HashMap::new(),
        // Rogues are proficient in DEX and INT saves (5e PHB).
        proficient_saves: HashSet::from([
            AbilityScoreType::Dexterity,
            AbilityScoreType::Intelligence,
        ]),
        condition_immunities: HashSet::new(),
        features: HashSet::new(),
        regen_per_round: 0,
        regen_suppressors: HashSet::new(),
        legendary_resistances: 0,
        has_evasion: true,
        has_uncanny_dodge: true,
        has_displacement: false,
        has_danger_sense: false,
        has_pack_tactics: false,
        has_magic_resistance: false,
        recharge_abilities: Vec::new(),
        legendary_actions_per_round: 0,
        has_extra_attack: false,
    }
});
