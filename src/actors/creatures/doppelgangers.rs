use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{DOPPELGANGER_MULTI, DOPPELGANGER_SLAM};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{Language, Size};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Doppelganger — CR 3 monstrosity. High AC (14) and 52 average HP
/// with a vanilla slam multiattack. The signature shapeshifter and
/// surprise-attack mechanics from MM aren't fully modeled (the engine
/// lacks a surprise round), but the strong stat line and charm
/// immunity keep doppelgangers feeling distinct from other CR-3
/// fighters. Pair with mages for a "blend in / attack from behind"
/// flavor encounter.
pub static DOPPELGANGER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&DOPPELGANGER_SLAM);
    actions.push(&*DOPPELGANGER_MULTI);
    CreatureTemplate {
        name: "Doppelganger",
        // 'D' was free (Dire Wolf is 'd'); use 'D' for doppelganger.
        glyph: 'D',
        ac: 14,
        // 8d8+16 = 52 average per MM.
        hitpoints: "8d8+16".parse().unwrap(),
        speed: 30.,
        strength: 11,
        intelligence: 11,
        dexterity: 18, // The marquee stat — drives initiative + slam.
        wisdom: 12,
        constitution: 14,
        charisma: 14,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        languages: HashSet::from([Language::Common]),
        cr: 3.0,
        size: Size::Medium,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        damage_modifiers: HashMap::new(),
        proficient_saves: HashSet::new(),
        // 5e: doppelgangers are immune to Charmed (they're the ones
        // doing the charming) — keeps the trope intact even without
        // shapeshift mechanics.
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
    }
});
