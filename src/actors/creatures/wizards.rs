use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::spells::{CAUSE_FEAR, FIRE_BOLT, MAGIC_MISSILE};
use crate::actors::actor_template::{CreatureTemplate, DamageAdjustments};
use crate::engine::types::{Language, Size};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Arcane spellcaster. INT-primary; Fire Bolt as the action-economy
/// staple, Magic Missile as a guaranteed-hit damage spike, Cause Fear
/// as a level-1 disabler. Light HP and AC — wizards stay at range or die.
pub static WIZARD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*FIRE_BOLT);
    actions.push(&*MAGIC_MISSILE);
    actions.push(&*CAUSE_FEAR);
    CreatureTemplate {
        name: "Wizard",
        glyph: 'M', // 'W' is wolf; 'M' for mage
        n_instances: 0,
        ac: 12,
        hitpoints: "2d6+2".parse().unwrap(),
        speed: 30.,
        strength: 8,
        intelligence: 16, // primary spellcasting ability
        dexterity: 12,
        wisdom: 13,
        constitution: 12,
        charisma: 10,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        languages: HashSet::from([Language::Common]),
        cr: 0.5,
        size: Size::Medium,
        actions,
        // 3 level-1 slots (Magic Missile / Cause Fear). No level-2 yet.
        spell_slots_by_level: vec![3],
        rolls_death_saves: false,
        damage_adjustments: DamageAdjustments::default(),
    }
});
