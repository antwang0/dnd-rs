use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::spells::{CAUSE_FEAR, FIRE_BOLT, MAGIC_MISSILE};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{Language, Size};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Wizard — INT-primary spellcaster PC alternative. Light HP, no armor,
/// but trades martial reliability for big-button spells. Fire Bolt as the
/// reliable cantrip damage option, Magic Missile as the auto-hit closer,
/// Cause Fear to lock down a single tough enemy. Mirror of Fighter as a
/// playable team-0 class.
pub static WIZARD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*FIRE_BOLT);
    actions.push(&*MAGIC_MISSILE);
    actions.push(&*CAUSE_FEAR);
    CreatureTemplate {
        name: "Wizard",
        glyph: 'M',
        n_instances: 0,
        ac: 12,
        hitpoints: "3d6+3".parse().unwrap(),
        speed: 30.,
        strength: 8,
        intelligence: 16, // primary spellcasting ability
        dexterity: 14,
        wisdom: 12,
        constitution: 12,
        charisma: 10,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        languages: HashSet::from([Language::Common, Language::Draconic]),
        cr: 1.0,
        size: Size::Medium,
        actions,
        // 4 level-1 (Magic Missile, Cause Fear), 2 level-2 (overcasts).
        spell_slots_by_level: vec![4, 2],
        rolls_death_saves: true,
        damage_modifiers: HashMap::new(),
    }
});
