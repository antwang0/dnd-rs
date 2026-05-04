use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::IMP_STING;
use crate::actions::spells::FIRE_BOLT;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{DamageMod, DamageType, Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Imp — diminutive devil with a poisoned sting and a Fire Bolt up its
/// sleeve. Tiny size (1×1 footprint) makes it the smallest creature in
/// the codebase. Devil heritage: Resistant to cold, Immune to fire and
/// poison. Pairs nicely with Fire Bolt — the Imp's own fire damage is
/// thematic, and fire-immune Imps are a callback to "the wizard fights
/// fire with fire and gets nowhere."
///
/// Stats roughly track MM Imp at CR 1 — high DEX, modest HP, melee +
/// ranged options. INT-primary so Fire Bolt actually hurts.
pub static IMP_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*IMP_STING);
    actions.push(&*FIRE_BOLT);
    CreatureTemplate {
        name: "Imp",
        glyph: 'I',
        n_instances: 0,
        ac: 13,
        hitpoints: "3d4+3".parse().unwrap(),
        speed: 20.,
        strength: 6,
        intelligence: 11,
        dexterity: 17,
        wisdom: 12,
        constitution: 13,
        charisma: 14,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(120)]),
        languages: HashSet::from([Language::Infernal, Language::Common]),
        cr: 1.0,
        size: Size::Tiny,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        // Devil heritage: fire / poison are at-home, cold is mostly
        // shrugged off, but bludgeoning / piercing / slashing land
        // normally (we don't track magical-weapons-only resistance yet).
        damage_mods: HashMap::from([
            (DamageType::Fire, DamageMod::Immune),
            (DamageType::Poison, DamageMod::Immune),
            (DamageType::Cold, DamageMod::Resistant),
        ]),
    }
});
