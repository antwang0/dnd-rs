use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::spells::{BURNING_HANDS, CAUSE_FEAR};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{Language, Size};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Mage — INT-based offensive caster. Burning Hands as a short-range
/// AoE, Cause Fear to lock down a single tough target. Light HP, low AC,
/// no melee — wants to stay at range and burn slots.
pub static MAGE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*BURNING_HANDS);
    actions.push(&*CAUSE_FEAR);
    CreatureTemplate {
        name: "Mage",
        glyph: 'M',
        n_instances: 0,
        ac: 12,
        hitpoints: "2d8+2".parse().unwrap(),
        speed: 30.,
        // We use WIS as the spell ability everywhere (spell_save_dc takes
        // an ability), so make WIS the Mage's high stat too — INT primary
        // would require threading INT through the spell_save_dc call sites.
        strength: 8,
        intelligence: 14,
        dexterity: 12,
        wisdom: 14,
        constitution: 12,
        charisma: 10,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        languages: HashSet::from([Language::Common, Language::Draconic]),
        cr: 0.5,
        size: Size::Medium,
        actions,
        // 3 level-1 slots — enough for a couple of Burning Hands or one
        // Burning + one Cause Fear.
        spell_slots_by_level: vec![3],
        rolls_death_saves: false,
        resistances: HashSet::new(),
        immunities: HashSet::new(),
        vulnerabilities: HashSet::new(),
    }
});
