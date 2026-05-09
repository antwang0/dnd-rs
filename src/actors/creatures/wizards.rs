use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::spells::{CAUSE_FEAR, FIRE_BOLT, MAGIC_MISSILE};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Apprentice wizard — INT-primary in 5e, but our spells key off WIS for
/// uniformity. Fire Bolt is the at-will damage cantrip, Magic Missile the
/// reliable level-1 burst. Light HP, medium AC, no melee — kite-and-shoot
/// pattern shared with the skeleton archer.
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
        hitpoints: "2d6+2".parse().unwrap(),
        speed: 30.,
        strength: 8,
        intelligence: 14,
        dexterity: 12,
        wisdom: 13,
        constitution: 12,
        charisma: 11,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common]),
        cr: 0.5,
        size: Size::Medium,
        actions,
        // Two level-1 slots — enough for a couple of Magic Missiles per fight.
        spell_slots_by_level: vec![2],
        rolls_death_saves: false,
        damage_resistances: HashSet::new(),
        damage_immunities: HashSet::new(),
        damage_vulnerabilities: HashSet::new(),
    }
});
