use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    DRAGON_BITE, DRAGON_CLAW, DRAGON_FIRE_BREATH, DRAGON_MULTI, FRIGHTFUL_PRESENCE,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{
    AbilityScoreType, DamageModifier, DamageType, Language, Size, SpecialSense,
};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Adult Red Dragon — CR 17 dragon, the marquee boss profile. Huge size
/// (4×4 footprint), AC 19, ~256 average HP. Three action lanes:
/// - Multiattack (3x claws against a single target) — the bursty melee
///   lane (Bite + 2 Claws RAW collapsed to 3 claws within the existing
///   single-sub-attack Multiattack scaffold).
/// - Single Bite — high-damage piercing with a 4d6 fire rider.
/// - Fire Breath — 60-ft cone (radius-6 burst) of 18d6 fire, DEX-save half.
///
/// The dragon is immune to fire and frightened — its iconic Frightful
/// Presence bonus action puts the Frightened condition on every nearby
/// non-immune enemy for 3 rounds. Has full proficient saves on DEX,
/// CON, WIS, CHA (Legendary save profile in MM).
pub static ADULT_RED_DRAGON_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*DRAGON_MULTI);
    actions.push(&*DRAGON_BITE);
    actions.push(&DRAGON_CLAW);
    actions.push(&*DRAGON_FIRE_BREATH);
    actions.push(&*FRIGHTFUL_PRESENCE);
    CreatureTemplate {
        name: "Adult Red Dragon",
        // 'D' is taken by other 'D'-glyphs in the pool; 'R' for "red dragon"
        // is distinctive and free.
        glyph: 'R',
        ac: 19,
        // 19d12+133 = 256 average per MM (CR 17).
        hitpoints: "19d12+133".parse().unwrap(),
        speed: 40.,
        strength: 27,
        intelligence: 16,
        dexterity: 10,
        wisdom: 13,
        constitution: 25,
        charisma: 21,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([
            SpecialSense::Blindsight(60),
            SpecialSense::Darkvision(120),
        ]),
        languages: HashSet::from([Language::Common, Language::Draconic]),
        cr: 17.0,
        size: Size::Large,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        damage_modifiers: HashMap::from([
            // Fire immunity is the dragon's signature defense.
            (DamageType::Fire, DamageModifier::Immunity),
        ]),
        // 5e Adult Red Dragon proficient saves: DEX, CON, WIS, CHA.
        proficient_saves: HashSet::from([
            AbilityScoreType::Dexterity,
            AbilityScoreType::Constitution,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]),
        // 5e: dragons are immune to Frightened (they fear nothing) and
        // Charmed (their wills are too strong).
        condition_immunities: HashSet::from([Condition::Frightened, Condition::Charmed]),
        features: HashSet::new(),
        regen_per_round: 0,
        regen_suppressors: HashSet::new(),
        // 5e Legendary Resistance (3/Day) — RAW per MM. Lets the dragon
        // shrug off a mid-fight Hold Monster / Banishment / Slow.
        legendary_resistances: 3,
        has_evasion: false,
        has_uncanny_dodge: false,
    }
});
