use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::spells::{BURNING_HANDS, FALSE_LIFE, MAGIC_MISSILE, SACRED_FLAME};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{Language, Size};
use std::collections::HashSet;
use std::sync::LazyLock;

/// INT-primary spellcaster. Magic Missile is the bread-and-butter ranged
/// damage option (auto-hit force damage), False Life provides a temp HP
/// buffer between encounters, Sacred Flame stands in as a damage cantrip
/// substitute (we don't have Fire Bolt yet). Light HP and AC — they want
/// to hang back.
pub static WIZARD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*MAGIC_MISSILE);
    actions.push(&*FALSE_LIFE);
    actions.push(&*BURNING_HANDS);
    actions.push(&*SACRED_FLAME);
    CreatureTemplate {
        name: "Wizard",
        glyph: 'M',
        n_instances: 0,
        ac: 12,
        hitpoints: "2d6+2".parse().unwrap(),
        speed: 30.,
        strength: 8,
        intelligence: 16, // primary spellcasting ability
        dexterity: 14,
        wisdom: 10,
        constitution: 12,
        charisma: 10,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        languages: HashSet::from([Language::Common, Language::Draconic]),
        cr: 0.5,
        size: Size::Medium,
        actions,
        // 4 level-1 slots (Magic Missile / False Life). No level-2 yet.
        spell_slots_by_level: vec![4],
        rolls_death_saves: false,
        resistances: HashSet::new(),
        vulnerabilities: HashSet::new(),
        immunities: HashSet::new(),
    }
});
