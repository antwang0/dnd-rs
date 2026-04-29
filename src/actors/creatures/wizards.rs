use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::spells::{BURNING_HANDS, MAGIC_MISSILE};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, Language, Size};
use std::collections::HashSet;
use std::sync::LazyLock;

/// INT-primary blaster. No melee fallback so positioning matters: stay
/// out of reach, use Magic Missile (auto-hit, level-1 slot) and Burning
/// Hands (cone AoE, level-1 slot). Lower HP than a cleric since the
/// wizard's defenses are kiting + spells, not armor.
pub static WIZARD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*MAGIC_MISSILE);
    actions.push(&*BURNING_HANDS);
    CreatureTemplate {
        name: "Wizard",
        glyph: 'M',
        n_instances: 0,
        ac: 12,
        hitpoints: "2d6+2".parse().unwrap(),
        speed: 30.,
        strength: 8,
        intelligence: 15, // primary spellcasting ability
        dexterity: 14,
        wisdom: 10,
        constitution: 12,
        charisma: 10,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        languages: HashSet::from([Language::Common]),
        cr: 0.25,
        size: Size::Medium,
        actions,
        // 4 level-1 slots is generous for CR 1/4 but lets the wizard
        // actually function in an encounter without burning the slot
        // budget on turn one.
        spell_slots_by_level: vec![4],
        rolls_death_saves: false,
        damage_resistances: HashSet::new(),
        damage_immunities: HashSet::new(),
        damage_vulnerabilities: HashSet::new(),
        // Wizard save proficiencies: INT / WIS (PHB).
        save_proficiencies: HashSet::from([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
        ]),
    }
});
