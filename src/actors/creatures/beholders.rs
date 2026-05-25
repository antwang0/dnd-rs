use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{BEHOLDER_EYE_RAY, BITE};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{AbilityScoreType, Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Beholder — CR 13 aberration. The marquee floating eye-monstrosity:
/// a generic 4d8 force Eye Ray as the ranged option, a fallback Bite
/// for melee. RAW the beholder fires multiple distinct eye rays per
/// turn (with named riders like sleep, paralysis, fear); we collapse
/// the ten rays into a single force-typed Eye Ray as a clean stand-in,
/// so the AI's action picker treats it as a long-reach single-target
/// attack without juggling sub-attack lookup logic. Immune to prone
/// (it floats) — modeled via a Prone condition immunity.
pub static BEHOLDER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*BITE);
    actions.push(&*BEHOLDER_EYE_RAY);
    CreatureTemplate {
        name: "Beholder",
        // 'B' is mostly free in our glyph pool (Bandit is 'b', Banshee
        // 'a', Berserker 'B' — already taken). Use 'O' (for oculus /
        // central eye) which is open.
        glyph: 'O',
        ac: 18,
        // 19d10+76 = 180 average per MM CR 13.
        hitpoints: "19d10+76".parse().unwrap(),
        speed: 0.,
        strength: 10,
        intelligence: 17,
        dexterity: 14,
        wisdom: 15,
        constitution: 18,
        charisma: 17,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(120)]),
        languages: HashSet::from([Language::Common, Language::DeepSpeech, Language::Undercommon]),
        cr: 13.0,
        size: Size::Large,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        damage_modifiers: HashMap::new(),
        proficient_saves: HashSet::from([
            AbilityScoreType::Constitution,
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
        ]),
        // Beholders float — they can't be knocked Prone.
        condition_immunities: HashSet::from([Condition::Prone]),
        features: HashSet::new(),
        regen_per_round: 0,
        regen_suppressors: HashSet::new(),
        legendary_resistances: 0,
        has_evasion: false,
        has_uncanny_dodge: false,
    }
});
