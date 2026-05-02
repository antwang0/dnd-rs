use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::spells::{BURNING_HANDS, MAGIC_MISSILE, SACRED_BURST, SACRED_FLAME};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, DamageType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Apprentice-tier arcanist: cantrips for rounds, Magic Missile for
/// the burst-down opener. INT-primary stats and a small leveled-slot
/// pool. Distinct from Cleric in that Magic Missile auto-hits — it
/// bypasses the AC + LOS-passes-but-target-misses problem.
///
/// Spellcasting ability is treated as WIS for save DC convenience
/// today; a future "spellcasting_ability" field on the template will
/// disambiguate (Wizards should use INT in 5e RAW).
pub static WIZARD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*SACRED_FLAME);
    actions.push(&*SACRED_BURST);
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
        intelligence: 16, // primary spellcasting in 5e RAW
        dexterity: 12,
        wisdom: 14,       // we still derive spell DC from WIS for now
        constitution: 12,
        charisma: 10,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(30)]),
        languages: HashSet::from([Language::Common]),
        cr: 0.5,
        size: Size::Medium,
        actions,
        // 4 level-1 slots — enough for several Magic Missiles per fight.
        spell_slots_by_level: vec![4],
        rolls_death_saves: false,
        resistances: HashSet::new(),
        immunities: HashSet::new(),
        vulnerabilities: HashSet::from([DamageType::Bludgeoning]),
        // Wizard: INT + WIS proficient saves (5e RAW).
        save_proficiencies: HashSet::from([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
        ]),
    }
});
