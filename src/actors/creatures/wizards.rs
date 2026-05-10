use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::spells::{
    BLINDNESS, BURNING_HANDS, CAUSE_FEAR, FIRE_BOLT, MAGIC_MISSILE, SHIELD, WEB,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Squishy INT-caster. Fire Bolt as the at-will ranged option, Magic
/// Missile and Burning Hands as level-1 nuke / AoE. Stat shape mirrors
/// the cleric (low HP, medium AC, tunes around ranged spell attacks)
/// but uses INT as the spellcasting ability so a separate save DC and
/// attack mod come into play.
pub static WIZARD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*FIRE_BOLT);
    actions.push(&*MAGIC_MISSILE);
    actions.push(&*BURNING_HANDS);
    actions.push(&*CAUSE_FEAR);
    actions.push(&*WEB);
    actions.push(&*BLINDNESS);
    actions.push(&*SHIELD);
    CreatureTemplate {
        name: "Wizard",
        // 'M' (mage) — keeps 'W' free for Wolf, which already claims it.
        glyph: 'M',
        ac: 12,
        hitpoints: "2d6+2".parse().unwrap(),
        speed: 30.,
        strength: 8,
        intelligence: 16, // primary spellcasting ability
        dexterity: 14,
        wisdom: 11,
        constitution: 12,
        charisma: 10,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common]),
        cr: 0.5,
        size: Size::Medium,
        actions,
        // 3 level-1 slots — typical level-2 wizard loadout. Plus 1
        // level-2 for Web / Blindness.
        spell_slots_by_level: vec![3, 1],
        rolls_death_saves: false,
        damage_modifiers: HashMap::new(),
        // Wizards are proficient in INT and WIS saves (5e PHB).
        proficient_saves: HashSet::from([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
        ]),
        condition_immunities: HashSet::new(),
        features: HashSet::new(),
    }
});
