use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{FIRE_BOLT, IMP_STING};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense};
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
        creature_type: CreatureType::Fiend,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        damage_modifiers: HashMap::from([
            (DamageType::Fire, DamageModifier::Immunity),
            (DamageType::Poison, DamageModifier::Immunity),
            (DamageType::Cold, DamageModifier::Resistance),
        ]),
        proficient_saves: HashSet::new(),
        condition_immunities: HashSet::new(),
        features: HashSet::new(),
        regen_per_round: 0,
        regen_suppressors: HashSet::new(),
        legendary_resistances: 0,
        has_evasion: false,
        has_uncanny_dodge: false,
        has_displacement: false,
        has_danger_sense: false,
        has_pack_tactics: false,
        has_magic_resistance: true,
        recharge_abilities: Vec::new(),
        legendary_actions_per_round: 0,
        has_extra_attack: false,
        brutal_critical_dice: 0,
        crit_threshold: 20,
        has_lucky: false,
        has_aura_of_protection: false,
        has_aura_of_courage: false,
    }
});
