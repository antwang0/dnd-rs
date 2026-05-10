use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{CUNNING_ACTION, SHORTBOW, SNEAK_ATTACK_DAGGER};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{Language, Size};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Rogue — DEX-based PC class. Headline features: Sneak Attack (extra
/// 1d6 when attacking with advantage or with an ally adjacent to the
/// target), Cunning Action (bonus-action Dash), and a shortbow for
/// safe ranged plinking. Rolls death saves like any PC.
///
/// Stats reflect a level-3 fighter equivalent with DEX-build defaults:
/// 21 HP (3d8+3), AC 14 (leather + DEX), DEX 16, CON 12.
pub static ROGUE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*SNEAK_ATTACK_DAGGER);
    actions.push(&*SHORTBOW);
    actions.push(&*CUNNING_ACTION);
    CreatureTemplate {
        name: "Rogue",
        glyph: 'R',
        n_instances: 0,
        ac: 14,
        hitpoints: "3d8+3".parse().unwrap(),
        speed: 30.,
        strength: 10,
        intelligence: 12,
        dexterity: 16,
        wisdom: 11,
        constitution: 12,
        charisma: 12,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        languages: HashSet::from([Language::Common, Language::ThievesCant]),
        cr: 1.0,
        size: Size::Medium,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: true,
        damage_resistances: HashSet::new(),
        damage_immunities: HashSet::new(),
        damage_vulnerabilities: HashSet::new(),
        condition_immunities: HashSet::new(),
    }
});
