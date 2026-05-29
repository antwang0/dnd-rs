use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    DEATH_KNIGHT_HELLFIRE_ORB, DEATH_KNIGHT_LONGSWORD, DEATH_KNIGHT_MULTI,
};
use crate::actions::spells::{
    DISPEL_MAGIC, FIRE_BOLT, FIREBALL, HOLD_MONSTER, MAGIC_MISSILE, MIRROR_IMAGE, SHIELD,
    WALL_OF_FIRE,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense,
};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Death Knight — CR 17 undead boss. RAW: a fallen paladin twisted into
/// an unholy half-mage / half-fighter. Three-swing longsword multi
/// (each swing rides a 4d8 necrotic empowerment), a Hellfire Orb burst
/// (10d8 fire DEX-save), and a small spell list — Magic Missile / Fire
/// Bolt at-will, Fireball + Wall of Fire as the fire-themed evocation
/// lane, Hold Monster as the boss-controller's lock. Mirrors the
/// boss-tier slot loadout used by the Lich (4/3/3/3/3/2/2/2/2) but
/// trimmed to a tight ~10-spell list so the AI doesn't drown in
/// options.
///
/// Defensive profile: necrotic + poison immunity (standard undead),
/// resistance to non-magical B/P/S, condition immunity to poison /
/// charm / fright / exhaustion.
pub static DEATH_KNIGHT_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    // Melee + multi
    actions.push(&*DEATH_KNIGHT_LONGSWORD);
    actions.push(&*DEATH_KNIGHT_MULTI);
    // Signature burst
    actions.push(&*DEATH_KNIGHT_HELLFIRE_ORB);
    // Spell list — keep tight. Death Knights cast as a 7th-level paladin
    // RAW (Oathbreaker, fire-themed). We pull from the fire / abjuration
    // lanes to match.
    actions.push(&*FIRE_BOLT);
    actions.push(&*SHIELD);
    actions.push(&*MAGIC_MISSILE);
    actions.push(&*MIRROR_IMAGE);
    actions.push(&*FIREBALL);
    actions.push(&*DISPEL_MAGIC);
    actions.push(&*WALL_OF_FIRE);
    actions.push(&*HOLD_MONSTER);
    CreatureTemplate {
        name: "Death Knight",
        // 'K' is free (Knight is uppercase too — but Knight uses 'k').
        // Keep 'K' for the boss undead variant.
        glyph: 'K',
        ac: 20,
        // 18d8+90 = 171 average per MM.
        hitpoints: "18d8+90".parse().unwrap(),
        speed: 30.,
        strength: 20,
        intelligence: 12,
        dexterity: 11,
        wisdom: 16,
        constitution: 20,
        charisma: 18, // drives the spell save DC
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(120)]),
        languages: HashSet::from([Language::Common, Language::Infernal]),
        cr: 17.0,
        size: Size::Medium,
        creature_type: CreatureType::Undead,
        actions,
        // CR 17 paladin-caster equivalent. Slimmer than the Lich's apex
        // loadout — the Death Knight leans on its Hellfire Orb + multi
        // pattern rather than a deep slot economy.
        spell_slots_by_level: vec![4, 3, 3, 1],
        rolls_death_saves: false,
        damage_modifiers: HashMap::from([
            (DamageType::Necrotic, DamageModifier::Immunity),
            (DamageType::Poison, DamageModifier::Immunity),
            // Resistance to non-magical B/P/S (we collapse to straight
            // resistance like other undead in this codebase).
            (DamageType::Bludgeoning, DamageModifier::Resistance),
            (DamageType::Piercing, DamageModifier::Resistance),
            (DamageType::Slashing, DamageModifier::Resistance),
        ]),
        // Boss-tier saves: prof in DEX / WIS / CHA per MM.
        proficient_saves: HashSet::from([
            AbilityScoreType::Dexterity,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]),
        // Standard undead condition immunities + Exhausted (5e Death
        // Knight is "tireless" RAW; we approximate via condition
        // immunity to Exhausted).
        condition_immunities: HashSet::from([
            Condition::Poisoned,
            Condition::Charmed,
            Condition::Frightened,
            Condition::Exhausted,
        ]),
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
        legendary_actions_per_round: 3,
        has_extra_attack: true,
        brutal_critical_dice: 0,
        crit_threshold: 20,
        has_lucky: false,
    }
});
