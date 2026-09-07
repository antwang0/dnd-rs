use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    BEHIR_BITE, BEHIR_CONSTRICT, BEHIR_LIGHTNING_BREATH, BEHIR_MULTI,
    SWALLOW_BONUS,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{
    CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense,
};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Behir — CR 11 huge monstrosity. Lightning-breath serpent: bite +
/// constrict CompoundAttack on melee (3d10+6 piercing then 2d10+6
/// bludgeoning + 2d10 slashing on the same target), plus a
/// recharge-5/6 lightning breath (12d10, burst-3 / range-5,
/// DC 16 DEX) for the opener. Lightning-immune so a chain-lightning
/// ally cast can't friendly-fire it; standalone bite + constrict are
/// also exposed so the AI has a graceful fallback when the breath is
/// on cooldown and the multi can't reach.
pub static BEHIR_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&BEHIR_BITE);
    actions.push(&BEHIR_CONSTRICT);
    actions.push(&BEHIR_LIGHTNING_BREATH);
    actions.push(&*BEHIR_MULTI);
    actions.push(&SWALLOW_BONUS);
    CreatureTemplate {
        name: "Behir",
        // 'B' for Behir — capital because Huge; collides with Bear
        // but the Huge serpent vs Large bear distinction is clear
        // in context (and on the map by footprint size).
        glyph: 'B',
        ac: 17,
        hitpoints: "16d12+64".parse().unwrap(),
        // RAW: 50 ft walk, 40 ft climb. The engine collapses to one
        // ground speed.
        speed: 50.,
        strength: 23,
        intelligence: 7,
        dexterity: 16,
        wisdom: 14,
        constitution: 18,
        charisma: 12,
        senses: HashSet::from([SpecialSense::Darkvision(90)]),
        languages: HashSet::from([Language::Draconic]),
        cr: 11.0,
        size: Size::Huge,
        creature_type: CreatureType::Monstrosity,
        actions,
        // Lightning-immune — the breath weapon's own pool feeds back
        // into the serpent's hide, so it's untouched by its own
        // discharge AND by ally lightning ambushes.
        damage_modifiers: HashMap::from([(DamageType::Lightning, DamageModifier::Immunity)]),
        // RAW recharge: lightning breath comes back on a d6 ≥ 5 at
        // the start of each turn. Standard `"breath_weapon"` pool key
        // shared with the dragon family — only one breath per recharge
        // window across every breath-bearing creature in the engine.
        recharge_abilities: vec![("breath_weapon", 5)],
        swallow: Some(&crate::actions::monster_attacks::BEHIR_SWALLOW),
        ..CreatureTemplate::defaults()
    }
});
