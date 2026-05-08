use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::SCIMITAR;
use crate::actors::actor_template::{CreatureTemplate, DamageAdjustments};
use crate::engine::types::{Language, Size};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Fighter — the simplest player class. Heavy armor, decent HP, one
/// martial weapon (scimitar — STR-based slashing) and the standard
/// movement actions. No spells. The headline distinction from monsters
/// is `rolls_death_saves: true` — at 0 HP a Fighter enters the dying
/// state and rolls saves on each of their turns instead of dropping
/// outright.
///
/// Stats are roughly a level-3 fighter: 24 HP (3d10+6), AC 16 from
/// chain mail, STR 16 (the standard "strength build" defaults).
pub static FIGHTER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*SCIMITAR);
    CreatureTemplate {
        name: "Fighter",
        glyph: 'F',
        n_instances: 0,
        ac: 16,
        hitpoints: "3d10+6".parse().unwrap(),
        speed: 30.,
        strength: 16,
        intelligence: 10,
        dexterity: 12,
        wisdom: 11,
        constitution: 14,
        charisma: 10,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        languages: HashSet::from([Language::Common]),
        cr: 1.0,
        size: Size::Medium,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: true,
        damage_adjustments: DamageAdjustments::default(),
    }
});
