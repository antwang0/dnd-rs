use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    PIT_FIEND_BITE, PIT_FIEND_CLAW, PIT_FIEND_FEAR_AURA, PIT_FIEND_MULTI,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense,
};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Pit Fiend — CR 20 archdevil boss. Heavy melee burst (bite + 2 claws
/// per Action), a fear aura that disables half the party on round 1,
/// and the standard devil envelope of fire immunity + non-physical
/// resistance. The marquee end-game boss for an adult party: even with
/// proper buffs, the fear aura alone neutralizes most of the front
/// line, and the multi-bite/claw burst is built to one-shot squishies.
///
/// Stats target the MM pit fiend: 300 HP, AC 19, STR-primary, immune
/// to fire / poison damage, immune to Poisoned / Charmed / Frightened.
pub static PIT_FIEND_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&PIT_FIEND_BITE);
    actions.push(&PIT_FIEND_CLAW);
    actions.push(&*PIT_FIEND_MULTI);
    actions.push(&*PIT_FIEND_FEAR_AURA);
    CreatureTemplate {
        name: "Pit Fiend",
        // 'F' for Fiend (uppercase to distinguish from 'f' frost-something).
        glyph: 'F',
        ac: 19,
        // 26d10+156 ≈ 300 average per the MM Pit Fiend stat block.
        hitpoints: "26d10+156".parse().unwrap(),
        speed: 30.,
        strength: 26,
        intelligence: 22,
        dexterity: 14,
        wisdom: 18,
        constitution: 24,
        charisma: 24, // spell save DC anchor / fear aura DC
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([
            SpecialSense::Truesight(120),
        ]),
        languages: HashSet::from([Language::Infernal, Language::Common]),
        cr: 20.0,
        size: Size::Large,
        creature_type: CreatureType::Fiend,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        // MM Pit Fiend: immune to fire + poison; resistant to cold +
        // non-magical bludgeoning / piercing / slashing. We omit the
        // magical-vs-mundane resistance distinction (we don't track it).
        damage_modifiers: HashMap::from([
            (DamageType::Fire, DamageModifier::Immunity),
            (DamageType::Poison, DamageModifier::Immunity),
            (DamageType::Cold, DamageModifier::Resistance),
        ]),
        // MM Pit Fiend proficient saves: DEX / CON / WIS.
        proficient_saves: HashSet::from([
            AbilityScoreType::Dexterity,
            AbilityScoreType::Constitution,
            AbilityScoreType::Wisdom,
        ]),
        // Devil condition immunity envelope: can't be poisoned, charmed,
        // or frightened — the latter pairing with the fear-aura is the
        // marquee "you can't fight back" interaction.
        condition_immunities: HashSet::from([
            Condition::Poisoned,
            Condition::Charmed,
            Condition::Frightened,
        ]),
        features: HashSet::new(),
        regen_per_round: 0,
        regen_suppressors: HashSet::new(),
        // 5e Legendary Resistance (3/Day) — RAW per MM. Routine for
        // a CR-20 archdevil boss.
        legendary_resistances: 3,
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
        has_brave: false,
        has_aura_of_protection: false,
        has_aura_of_courage: false,
        has_savage_attacks: false,
        has_dwarven_resilience: false,
        has_gnome_cunning: false,
        draconic_ancestry: None,
        sorcery_points: 0,
    }
});
