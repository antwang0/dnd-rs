use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{HILL_GIANT_BOULDER, HILL_GIANT_GREATCLUB};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{Language, Size};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Hill Giant — CR 5 giant. Pure brute: enormous HP pool, big-hit
/// greatclub at reach 2 (10 ft) or a thrown boulder at reach 24 (60 ft)
/// when the targets are out of melee. No rider effects; the giant's
/// threat profile is "hits very hard in either lane and shrugs off
/// physical damage by having too much HP to whittle down quickly."
/// Charmed-immune because we treat giants as resistant to most
/// mind-bending control spells (RAW giants aren't charm-immune across
/// the board but the cleric / wizard charm spells tend to fail on
/// large brutes; keeping the immunity for engine simplicity).
pub static HILL_GIANT_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&HILL_GIANT_GREATCLUB);
    actions.push(&HILL_GIANT_BOULDER);
    CreatureTemplate {
        name: "Hill Giant",
        // 'J' (uppercase) is free; uppercase 'G' is taken by goblin /
        // gargoyle. Use 'J' for "Jotun" / giant.
        glyph: 'J',
        ac: 13,
        // 10d12+40 = 105 average per MM. Massive CR 5 HP pool.
        hitpoints: "10d12+40".parse().unwrap(),
        speed: 40.,
        strength: 21,
        intelligence: 5,
        dexterity: 8,
        wisdom: 9,
        constitution: 19,
        charisma: 6,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        languages: HashSet::from([Language::Giant]),
        cr: 5.0,
        size: Size::Huge,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        damage_modifiers: HashMap::new(),
        proficient_saves: HashSet::new(),
        // Giants brush off the standard "you fall asleep" / "you're
        // charmed" spells in our pool. Sleep already breaks against
        // bigger HP totals; the charm immunity keeps Charm Person /
        // Hold Monster from neutralizing the giant outright.
        condition_immunities: HashSet::from([Condition::Charmed]),
        features: HashSet::new(),
        regen_per_round: 0,
        regen_suppressors: HashSet::new(),
        legendary_resistances: 0,
        has_evasion: false,
        has_uncanny_dodge: false,
        has_displacement: false,
        has_danger_sense: false,
        has_pack_tactics: false,
        has_magic_resistance: false,
        recharge_abilities: Vec::new(),
        legendary_actions_per_round: 0,
        has_extra_attack: true,
    }
});
