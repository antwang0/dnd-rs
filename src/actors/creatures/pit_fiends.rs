use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{PIT_FIEND_BITE, PIT_FIEND_CLAW, PIT_FIEND_MULTI};
use crate::engine::emanations::PIT_FIEND_FEAR_AURA;
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense,
};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Pit Fiend — CR 20 archdevil boss. Heavy melee burst (bite + 2 claws
/// per Action) and a **Fear Aura** — RAW's 20-foot emanation, billing a
/// DC 21 Wisdom save to anyone who starts a turn inside it, once, until
/// they make one. See `emanations::PIT_FIEND_FEAR_AURA`.
///
/// The aura used to be an *Action*: the fiend spent its whole turn to
/// frighten the room for ten rounds. That had the weight backwards in
/// both directions — RAW's aura costs the pit fiend nothing, so it
/// keeps the bite and two claws that are the CR-20 damage the fight is
/// about, and it lasts one round at a time rather than ten. What makes
/// it a boss aura is not the duration but the tax.
///
/// Stats target the MM pit fiend: 300 HP, AC 21, STR-primary, immune
/// to fire / poison damage, immune to Poisoned / Charmed / Frightened.
pub static PIT_FIEND_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&PIT_FIEND_BITE);
    actions.push(&PIT_FIEND_CLAW);
    actions.push(&*PIT_FIEND_MULTI);
    CreatureTemplate {
        name: "Pit Fiend",
        // 'F' for Fiend (uppercase to distinguish from 'f' frost-something).
        glyph: 'F',
        ac: 21,
        // 27d10+189 ≈ 300 average per the MM Pit Fiend stat block.
        hitpoints: "27d10+189".parse().unwrap(),
        // RAW speed line: Speed 30 ft., fly 60 ft.
        speed: 30.0,
        fly_speed: 60.0,
        strength: 26,
        dexterity: 14,
        constitution: 24,
        intelligence: 22,
        wisdom: 18,
        charisma: 24, // spell save DC anchor / fear aura DC
        senses: HashSet::from([SpecialSense::Truesight(120)]),
        languages: HashSet::from([Language::Infernal, Language::Common]),
        cr: 20.0,
        size: Size::Large,
        creature_type: CreatureType::Fiend,
        actions,
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
        // RAW **Fear Aura** — see the template docstring above and
        // `emanations::PIT_FIEND_FEAR_AURA`. The Frightened immunity two
        // rows up is what keeps a pair of pit fiends from cowing each
        // other on the rare board where they end up on opposite sides.
        emanations: std::slice::from_ref(&PIT_FIEND_FEAR_AURA),
        // 5e Legendary Resistance (3/Day) — RAW per MM. Routine for
        // a CR-20 archdevil boss.
        legendary_resistances: 3,
        has_magic_resistance: true,
        legendary_actions_per_round: 3,
        legendary_actions: crate::engine::legendary_actions::PIT_FIEND_LEGENDARY,
        has_extra_attack: true,
        // 5e **Devil's Sight** — "magical darkness doesn't impede this
        // devil's darkvision." Carried by every devil in the bestiary,
        // and the one thing in the game that sees through the Darkness
        // spell. Before the lighting layer existed the trait was
        // approximated as a generous darkvision radius, which was the
        // closest the engine could get to it and got the crucial half
        // exactly backwards: RAW darkvision is precisely what magical
        // darkness defeats.
        features: HashSet::from([
            crate::actions::class_features::DEVILS_SIGHT_TAG,
            // 5e **Magic Weapons**: "the devil's weapon attacks
            // are magical."
            crate::actions::class_features::MAGICAL_ATTACKS_TAG,
        ]),
        ..CreatureTemplate::defaults()
    }
});
