use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::spells::{FIRE_BOLT, MAGIC_MISSILE, SHIELD_SPELL};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, Language, Size};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Apprentice-flavored arcane caster. INT-primary; Fire Bolt as the
/// at-will damage option, Magic Missile as the level-1 damage spell,
/// Shield as a defensive reaction. Light HP, low AC, no weapon — meant
/// to play from a distance with pickets in front.
pub static WIZARD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*FIRE_BOLT);
    actions.push(&*MAGIC_MISSILE);
    actions.push(&*SHIELD_SPELL);
    CreatureTemplate {
        name: "Wizard",
        glyph: 'W',
        n_instances: 0,
        ac: 12,
        hitpoints: "2d6+2".parse().unwrap(),
        speed: 30.,
        strength: 8,
        intelligence: 14, // primary spellcasting ability
        dexterity: 12,
        wisdom: 10,
        constitution: 12,
        charisma: 10,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        languages: HashSet::from([Language::Common]),
        cr: 0.5,
        size: Size::Medium,
        actions,
        // 3 level-1 slots: Magic Missile + Shield reactions.
        spell_slots_by_level: vec![3],
        rolls_death_saves: false,
        resistances: HashSet::new(),
        vulnerabilities: HashSet::new(),
        immunities: HashSet::new(),
        condition_immunities: HashSet::new(),
        proficient_saves: HashSet::from([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
        ]),
    }
});
