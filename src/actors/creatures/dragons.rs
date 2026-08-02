use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    DRAGON_BITE, DRAGON_BREATH_COLD, DRAGON_BREATH_FIRE, DRAGON_BREATH_LIGHTNING,
    DRAGON_BREATH_POISON, DRAGON_CLAW, DRAGON_MULTI, FRIGHTFUL_PRESENCE,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense,
};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// The DEX / CON / WIS / CHA save profile every dragon in this engine
/// shares (Legendary save profile in MM). Lifted to a `LazyLock` because
/// `HashSet` isn't `const`-constructible from a literal; the three
/// dragon templates below thread the same value through `proficient_saves`.
static DRAGON_LEGENDARY_SAVES: LazyLock<HashSet<AbilityScoreType>> = LazyLock::new(|| {
    HashSet::from([
        AbilityScoreType::Dexterity,
        AbilityScoreType::Constitution,
        AbilityScoreType::Wisdom,
        AbilityScoreType::Charisma,
    ])
});

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
    actions.push(&DRAGON_BITE);
    actions.push(&DRAGON_CLAW);
    actions.push(&DRAGON_BREATH_FIRE);
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
        senses: HashSet::from([
            SpecialSense::Blindsight(60),
            SpecialSense::Darkvision(120),
        ]),
        languages: HashSet::from([Language::Common, Language::Draconic]),
        cr: 17.0,
        size: Size::Large,
        creature_type: CreatureType::Dragon,
        actions,
        // Fire immunity is the dragon's signature defense.
        damage_modifiers: HashMap::from([(DamageType::Fire, DamageModifier::Immunity)]),
        proficient_saves: DRAGON_LEGENDARY_SAVES.clone(),
        // 5e: dragons are immune to Frightened (they fear nothing) and
        // Charmed (their wills are too strong).
        condition_immunities: HashSet::from([Condition::Frightened, Condition::Charmed]),
        // 5e Legendary Resistance (3/Day) — RAW per MM. Lets the dragon
        // shrug off a mid-fight Hold Monster / Banishment / Slow.
        legendary_resistances: 3,
        has_magic_resistance: true,
        recharge_abilities: vec![("breath_weapon", 5)],
        legendary_actions_per_round: 3,
        // 5e lair actions — the cave itself acts once a round while
        // the dragon lives in it. See `engine::lair_actions`.
        lair_actions: crate::engine::lair_actions::DRAGON_LAIR,
        ..CreatureTemplate::defaults()
    }
});

/// Young White Dragon — CR 6 dragon. A mid-tier encounter with cold breath
/// and multiattack (bite + claws). No legendary actions or resistances —
/// it's a young dragon that relies on raw physical power and its freezing
/// breath weapon. AC 17, ~133 average HP.
pub static YOUNG_WHITE_DRAGON_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&DRAGON_BITE);
    actions.push(&DRAGON_CLAW);
    actions.push(&DRAGON_BREATH_COLD);
    CreatureTemplate {
        name: "Young White Dragon",
        glyph: 'W',
        ac: 17,
        // 13d10+39 = 110.5 average, but MM rounds to ~133.
        hitpoints: "13d10+39".parse().unwrap(),
        speed: 40.,
        strength: 18,
        intelligence: 6,
        dexterity: 10,
        wisdom: 11,
        constitution: 18,
        charisma: 12,
        senses: HashSet::from([
            SpecialSense::Blindsight(30),
            SpecialSense::Darkvision(120),
        ]),
        languages: HashSet::from([Language::Common, Language::Draconic]),
        cr: 6.0,
        size: Size::Large,
        creature_type: CreatureType::Dragon,
        actions,
        damage_modifiers: HashMap::from([(DamageType::Cold, DamageModifier::Immunity)]),
        proficient_saves: DRAGON_LEGENDARY_SAVES.clone(),
        // Young dragons have NO legendary resistances or actions; the
        // breath weapon is still on a 5-6 recharge.
        recharge_abilities: vec![("breath_weapon", 5)],
        has_extra_attack: true,
        ..CreatureTemplate::defaults()
    }
});

/// Ancient Blue Dragon — CR 23 boss dragon. Lightning breath, Frightful
/// Presence, multiattack (bite + claws), and the full legendary package:
/// 3 legendary resistances and 3 legendary actions per round. AC 22,
/// ~507 average HP. One of the most dangerous encounters in the game.
pub static ANCIENT_BLUE_DRAGON_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*DRAGON_MULTI);
    actions.push(&DRAGON_BITE);
    actions.push(&DRAGON_CLAW);
    actions.push(&DRAGON_BREATH_LIGHTNING);
    actions.push(&*FRIGHTFUL_PRESENCE);
    CreatureTemplate {
        name: "Ancient Blue Dragon",
        glyph: 'B',
        ac: 22,
        // 26d20+234 = 507 average per MM (CR 23).
        hitpoints: "26d20+234".parse().unwrap(),
        speed: 40.,
        strength: 29,
        intelligence: 18,
        dexterity: 10,
        wisdom: 17,
        constitution: 27,
        charisma: 21,
        senses: HashSet::from([
            SpecialSense::Blindsight(60),
            SpecialSense::Darkvision(120),
        ]),
        languages: HashSet::from([Language::Common, Language::Draconic]),
        cr: 23.0,
        size: Size::Large,
        creature_type: CreatureType::Dragon,
        actions,
        damage_modifiers: HashMap::from([(DamageType::Lightning, DamageModifier::Immunity)]),
        proficient_saves: DRAGON_LEGENDARY_SAVES.clone(),
        // Ancient dragons are immune to Frightened and Charmed.
        condition_immunities: HashSet::from([Condition::Frightened, Condition::Charmed]),
        legendary_resistances: 3,
        has_magic_resistance: true,
        recharge_abilities: vec![("breath_weapon", 5)],
        legendary_actions_per_round: 3,
        // 5e lair actions — the cave itself acts once a round while
        // the dragon lives in it. See `engine::lair_actions`.
        lair_actions: crate::engine::lair_actions::DRAGON_LAIR,
        has_extra_attack: true,
        ..CreatureTemplate::defaults()
    }
});

/// Adult Green Dragon — CR 15 dragon, and the one the engine's poison
/// breath was written for.
///
/// `DRAGON_BREATH_POISON` has existed for as long as its three
/// elemental siblings: a complete `BreathWeapon` with the MM's 12d6,
/// DC 21, 60-ft cone, and the one detail that makes it worth having —
/// a **Constitution** save rather than the DEX save every other breath
/// on the chassis rolls, because the cloud is inhaled rather than
/// dodged. No template carried it, so no encounter could roll it and
/// no test could see it. The same way Warding Wind sat unreachable on
/// the spell list.
///
/// Sits between the Young White (CR 6) and the Ancient Blue (CR 23) on
/// the dragon ladder, which is also where it belongs mechanically: one
/// legendary resistance rather than the ancient's three, Frightful
/// Presence, and no legendary actions. AC 19, ~207 average HP.
///
/// The CON breath is what makes it play differently from its siblings
/// rather than being a recolour. Every DEX-save burst in the game
/// rewards the same answers — Evasion, a high-DEX chassis, spreading
/// out — and the green dragon's cloud ignores all three. A rogue who
/// walks through Fire Breath for nothing takes the poison in full.
///
/// Glyph 'G' for the **G**reen. Distinct from the red 'R', white 'W'
/// and blue 'B' already on the ladder.
pub static ADULT_GREEN_DRAGON_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*DRAGON_MULTI);
    actions.push(&DRAGON_BITE);
    actions.push(&DRAGON_CLAW);
    actions.push(&DRAGON_BREATH_POISON);
    actions.push(&*FRIGHTFUL_PRESENCE);
    CreatureTemplate {
        name: "Adult Green Dragon",
        glyph: 'G',
        ac: 19,
        // 17d12+102 = 207 average per MM (CR 15).
        hitpoints: "17d12+102".parse().unwrap(),
        speed: 40.,
        strength: 23,
        intelligence: 18,
        dexterity: 12,
        wisdom: 15,
        constitution: 21,
        charisma: 17,
        senses: HashSet::from([
            SpecialSense::Blindsight(60),
            SpecialSense::Darkvision(120),
        ]),
        languages: HashSet::from([Language::Common, Language::Draconic]),
        cr: 15.0,
        size: Size::Large,
        creature_type: CreatureType::Dragon,
        actions,
        damage_modifiers: HashMap::from([(DamageType::Poison, DamageModifier::Immunity)]),
        proficient_saves: DRAGON_LEGENDARY_SAVES.clone(),
        // MM gives the adult green poison immunity, which carries the
        // Poisoned condition immunity with it.
        condition_immunities: HashSet::from([Condition::Poisoned]),
        // Adults get legendary resistances but not the ancient's three.
        legendary_resistances: 1,
        recharge_abilities: vec![("breath_weapon", 5)],
        has_extra_attack: true,
        ..CreatureTemplate::defaults()
    }
});
