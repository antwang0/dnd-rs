use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    FRIGHTFUL_PRESENCE, TARRASQUE_BITE, TARRASQUE_CLAW, TARRASQUE_MULTI, TARRASQUE_TAIL,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{
    AbilityScoreType, DamageModifier, DamageType, Size, SpecialSense,
};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Tarrasque — CR 30, the apex 5e creature. Gargantuan (4×4 footprint),
/// AC 25, ~676 average HP. Apex-tier defense profile:
/// - Resistant to bludgeoning / piercing / slashing from non-magical
///   weapons (we collapse to a flat physical-resistance for simplicity
///   since the engine doesn't track magic-weapon properties).
/// - Immune to fire and poison.
/// - Immune to Charmed / Frightened / Paralyzed / Poisoned (raw RAW).
///
/// Action lanes:
/// - **Multiattack** (1 bite + 2 claws + 1 tail) — the bursty 4-attack
///   melee combo, threat radius extending to 4 tiles.
/// - **Tarrasque Bite** — high-damage piercing standalone for when the
///   multi isn't worth the action (e.g. on a small target).
/// - **Tail Sweep** — Prone-on-hit lane to lock down ranged casters.
/// - **Frightful Presence** — bonus-action AoE fright on all nearby
///   non-immune enemies (reused from the dragon's loadout — the
///   tarrasque's aura is identical in MM RAW).
///
/// Regen 40/round (no suppressor — the tarrasque heals unconditionally).
pub static TARRASQUE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*TARRASQUE_MULTI);
    actions.push(&TARRASQUE_BITE);
    actions.push(&TARRASQUE_CLAW);
    actions.push(&*TARRASQUE_TAIL);
    actions.push(&*FRIGHTFUL_PRESENCE);
    CreatureTemplate {
        name: "Tarrasque",
        glyph: 'T',
        ac: 25,
        // 33d20+330 = ~676 average per MM (CR 30).
        hitpoints: "33d20+330".parse().unwrap(),
        speed: 40.,
        strength: 30,
        intelligence: 3,
        dexterity: 11,
        wisdom: 11,
        constitution: 30,
        charisma: 11,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([
            SpecialSense::Blindsight(120),
            SpecialSense::Tremorsense(120),
        ]),
        languages: HashSet::new(), // Tarrasques don't speak.
        cr: 30.0,
        size: Size::Gargantuan,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        damage_modifiers: HashMap::from([
            // Apex-tier elemental immunities.
            (DamageType::Fire, DamageModifier::Immunity),
            (DamageType::Poison, DamageModifier::Immunity),
            // Carapace shrugs off mundane weapon damage; we approximate
            // "non-magical bludgeoning / piercing / slashing" as a flat
            // physical resistance since the engine doesn't model magic
            // weapon properties.
            (DamageType::Bludgeoning, DamageModifier::Resistance),
            (DamageType::Piercing, DamageModifier::Resistance),
            (DamageType::Slashing, DamageModifier::Resistance),
        ]),
        // 5e Tarrasque has Legendary saves on every score (collapse to
        // proficient saves across the board for our model).
        proficient_saves: HashSet::from([
            AbilityScoreType::Strength,
            AbilityScoreType::Dexterity,
            AbilityScoreType::Constitution,
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]),
        condition_immunities: HashSet::from([
            Condition::Charmed,
            Condition::Frightened,
            Condition::Paralyzed,
            Condition::Poisoned,
        ]),
        features: HashSet::new(),
        // Regenerates 40 HP at the end of each of its turns. No
        // suppressor — the tarrasque regenerates unconditionally.
        regen_per_round: 40,
        regen_suppressors: HashSet::new(),
        // 5e Legendary Resistance (3/Day): three failed saves per long
        // rest are auto-promoted to passes. The Tarrasque needs these
        // to shrug off Power Word Kill / Banishment / Hold Monster from
        // the party's casters mid-fight.
        legendary_resistances: 3,
    }
});
