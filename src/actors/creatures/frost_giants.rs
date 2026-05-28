use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{FROST_GIANT_GREATAXE, FROST_GIANT_ROCK};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Frost Giant — CR 8 giant. Big-dice melee hitter with a long-range
/// boulder option, frost-themed damage immunity. Mechanically: a slower,
/// hardier Hill Giant — bigger HP pool, slashing greataxe instead of
/// bludgeoning club, and the cold immunity is the marquee shape (any
/// cold burst tickle from a wizard becomes a no-op against the frost
/// giant, mirroring the Fire Elemental's fire immunity).
pub static FROST_GIANT_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&FROST_GIANT_GREATAXE);
    actions.push(&FROST_GIANT_ROCK);
    CreatureTemplate {
        name: "Frost Giant",
        // 'G' as the canonical giant glyph (lower-case 'g' is goblin).
        glyph: 'G',
        ac: 15,
        // 12d12+60 ≈ 138 average per the MM Frost Giant stat block.
        hitpoints: "12d12+60".parse().unwrap(),
        speed: 40.,
        strength: 23,
        intelligence: 9,
        dexterity: 9,
        wisdom: 10,
        constitution: 21,
        charisma: 12,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        languages: HashSet::from([Language::Giant]),
        cr: 8.0,
        size: Size::Huge,
        creature_type: CreatureType::Giant,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        // 5e MM Frost Giant: immune to cold damage. We don't model the
        // "vulnerable to fire" line MM gives some frost variants — RAW
        // doesn't grant vulnerability, just immunity to cold.
        damage_modifiers: HashMap::from([(DamageType::Cold, DamageModifier::Immunity)]),
        // Frost Giant proficient saves: CON / WIS / CHA per MM.
        proficient_saves: HashSet::from([
            AbilityScoreType::Constitution,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]),
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
        has_magic_resistance: false,
        recharge_abilities: Vec::new(),
        legendary_actions_per_round: 0,
        has_extra_attack: true,
    }
});
