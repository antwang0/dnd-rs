use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{GNOLL_PACK_LORD_MULTI, LONGBOW};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Gnoll Pack Lord — CR 2 gnoll warband leader. Tougher and smarter than
/// a baseline gnoll, the pack lord wields a glaive with reach-2 and
/// swings it twice per Action via multiattack. Falls back to a longbow
/// when enemies stay at range. The higher STR (16) and CON (14) make it
/// a credible frontliner that can anchor a pack of CR 1/2 gnolls.
pub static GNOLL_PACK_LORD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*GNOLL_PACK_LORD_MULTI);
    actions.push(&LONGBOW);
    CreatureTemplate {
        name: "Gnoll Pack Lord",
        // 'L' for pack Lord — 'N' is taken by the base gnoll.
        glyph: 'L',
        ac: 15,
        hitpoints: "3d10+6".parse().unwrap(),
        speed: 30.,
        strength: 16,
        intelligence: 12,
        dexterity: 14,
        wisdom: 11,
        constitution: 14,
        charisma: 12,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common]),
        cr: 2.0,
        size: Size::Medium,
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
    }
});
