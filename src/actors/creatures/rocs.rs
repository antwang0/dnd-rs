use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{ROC_BEAK, ROC_MULTI, ROC_TALONS};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{AbilityScoreType, CreatureType, Size, Skill, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Roc — CR 11 huge beast. The legendary giant eagle: 4d8 beak + 4d6
/// talons CompoundAttack per Action. Sibling to Giant Eagle (CR 1) at
/// the apex of the avian ladder; fills the CR-11 slot alongside Behir on
/// the upper-mid pool. RAW the Roc is Gargantuan (20-ft footprint); we
/// drop it to Huge because the engine's spawn placement code clamps at
/// 3×3 cleanly and 4×4 hits edge cases on smaller maps.
///
/// Persistent Flying via the `Flying` condition install at creature
/// instantiation lives in `ApplyCondition` — for templates this is
/// expressed by setting the Flying condition on spawn via an action.
/// We skip that here since `Flying` is a movement modifier that the
/// engine's terrain isn't 3D enough to meaningfully use — the AI still
/// kites correctly without it.
pub static ROC_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&ROC_BEAK);
    actions.push(&ROC_TALONS);
    actions.push(&*ROC_MULTI);
    CreatureTemplate {
        name: "Roc",
        // 'K' (capital) — free in the huge beast slot; distinct from
        // 'E' (Giant Eagle) and 'R' (Red Dragon). 'K' for the Old
        // World "Rukh" spelling of Roc.
        glyph: 'K',
        ac: 15,
        // 14d12+56 ≈ 248 average per MM (CR 11).
        hitpoints: "14d12+56".parse().unwrap(),
        // RAW speed line: Speed 20 ft., fly 120 ft.
        speed: 20.0,
        fly_speed: 120.0,
        strength: 28,
        intelligence: 3,
        dexterity: 10,
        wisdom: 10,
        constitution: 20,
        charisma: 9,
        skills: HashSet::from([Skill::Perception]),
        senses: HashSet::from([SpecialSense::Darkvision(120)]),
        cr: 11.0,
        size: Size::Huge,
        creature_type: CreatureType::Beast,
        actions,
        // 5e Roc proficient saves: STR / DEX / CON / WIS per MM.
        proficient_saves: HashSet::from([
            AbilityScoreType::Strength,
            AbilityScoreType::Dexterity,
            AbilityScoreType::Constitution,
            AbilityScoreType::Wisdom,
        ]),
        // Rocs are too vast / brain-stunted to be charmed by mortal
        // magic; mirrors the Hill / Fire Giant Charmed immunity.
        condition_immunities: HashSet::from([Condition::Charmed]),
        has_extra_attack: true,
        ..CreatureTemplate::defaults()
    }
});
