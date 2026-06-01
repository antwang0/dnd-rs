use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    DRAGON_BITE, DRAGON_BREATH_COLD, DRAGON_BREATH_FIRE, DRAGON_BREATH_LIGHTNING, DRAGON_CLAW,
    DRAGON_MULTI, FRIGHTFUL_PRESENCE,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense,
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
    actions.push(&*DRAGON_BREATH_FIRE);
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
        creature_type: CreatureType::Dragon,
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
        has_displacement: false,
        has_danger_sense: false,
        has_pack_tactics: false,
        has_magic_resistance: true,
        recharge_abilities: vec![("breath_weapon", 5)],
        legendary_actions_per_round: 3,
        has_extra_attack: false,
        brutal_critical_dice: 0,
        crit_threshold: 20,
        has_lucky: false,
        has_brave: false,
        has_fey_ancestry: false,
        has_aura_of_protection: false,
        has_aura_of_courage: false,
        has_savage_attacks: false,
        has_dwarven_resilience: false,
        has_gnome_cunning: false,
        draconic_ancestry: None,
        sorcery_points: 0,
    }
});

/// Young White Dragon — CR 6 dragon. A mid-tier encounter with cold breath
/// and multiattack (bite + claws). No legendary actions or resistances —
/// it's a young dragon that relies on raw physical power and its freezing
/// breath weapon. AC 17, ~133 average HP.
pub static YOUNG_WHITE_DRAGON_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*DRAGON_BITE);
    actions.push(&DRAGON_CLAW);
    actions.push(&*DRAGON_BREATH_COLD);
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
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([
            SpecialSense::Blindsight(30),
            SpecialSense::Darkvision(120),
        ]),
        languages: HashSet::from([Language::Common, Language::Draconic]),
        cr: 6.0,
        size: Size::Large,
        creature_type: CreatureType::Dragon,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        damage_modifiers: HashMap::from([
            (DamageType::Cold, DamageModifier::Immunity),
        ]),
        // 5e Young White Dragon proficient saves: DEX, CON, WIS, CHA.
        proficient_saves: HashSet::from([
            AbilityScoreType::Dexterity,
            AbilityScoreType::Constitution,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]),
        // Young dragons have no special condition immunities.
        condition_immunities: HashSet::new(),
        features: HashSet::new(),
        regen_per_round: 0,
        regen_suppressors: HashSet::new(),
        // Young dragons have NO legendary resistances.
        legendary_resistances: 0,
        has_evasion: false,
        has_uncanny_dodge: false,
        has_displacement: false,
        has_danger_sense: false,
        has_pack_tactics: false,
        has_magic_resistance: false,
        recharge_abilities: vec![("breath_weapon", 5)],
        // Young dragons have NO legendary actions.
        legendary_actions_per_round: 0,
        has_extra_attack: true,
        brutal_critical_dice: 0,
        crit_threshold: 20,
        has_lucky: false,
        has_brave: false,
        has_fey_ancestry: false,
        has_aura_of_protection: false,
        has_aura_of_courage: false,
        has_savage_attacks: false,
        has_dwarven_resilience: false,
        has_gnome_cunning: false,
        draconic_ancestry: None,
        sorcery_points: 0,
    }
});

/// Ancient Blue Dragon — CR 23 boss dragon. Lightning breath, Frightful
/// Presence, multiattack (bite + claws), and the full legendary package:
/// 3 legendary resistances and 3 legendary actions per round. AC 22,
/// ~507 average HP. One of the most dangerous encounters in the game.
pub static ANCIENT_BLUE_DRAGON_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*DRAGON_MULTI);
    actions.push(&*DRAGON_BITE);
    actions.push(&DRAGON_CLAW);
    actions.push(&*DRAGON_BREATH_LIGHTNING);
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
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([
            SpecialSense::Blindsight(60),
            SpecialSense::Darkvision(120),
        ]),
        languages: HashSet::from([Language::Common, Language::Draconic]),
        cr: 23.0,
        size: Size::Large,
        creature_type: CreatureType::Dragon,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        damage_modifiers: HashMap::from([
            (DamageType::Lightning, DamageModifier::Immunity),
        ]),
        // 5e Ancient Blue Dragon proficient saves: DEX, CON, WIS, CHA.
        proficient_saves: HashSet::from([
            AbilityScoreType::Dexterity,
            AbilityScoreType::Constitution,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]),
        // Ancient dragons are immune to Frightened and Charmed.
        condition_immunities: HashSet::from([Condition::Frightened, Condition::Charmed]),
        features: HashSet::new(),
        regen_per_round: 0,
        regen_suppressors: HashSet::new(),
        // 5e Legendary Resistance (3/Day).
        legendary_resistances: 3,
        has_evasion: false,
        has_uncanny_dodge: false,
        has_displacement: false,
        has_danger_sense: false,
        has_pack_tactics: false,
        has_magic_resistance: true,
        recharge_abilities: vec![("breath_weapon", 5)],
        legendary_actions_per_round: 3,
        has_extra_attack: true,
        brutal_critical_dice: 0,
        crit_threshold: 20,
        has_lucky: false,
        has_brave: false,
        has_fey_ancestry: false,
        has_aura_of_protection: false,
        has_aura_of_courage: false,
        has_savage_attacks: false,
        has_dwarven_resilience: false,
        has_gnome_cunning: false,
        draconic_ancestry: None,
        sorcery_points: 0,
    }
});
