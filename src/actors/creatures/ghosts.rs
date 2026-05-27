use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{GHOST_HORRIFYING_VISAGE, GHOST_WITHERING_TOUCH};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{DamageModifier, DamageType, Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Ghost — CR 4 incorporeal undead. RAW: a wandering soul with a
/// withering necrotic touch and a Horrifying Visage burst that
/// frightens nearby living creatures. We model the incorporeal-
/// movement clause via straight resistance to non-magical B/P/S (the
/// 5e formula is "resistance to non-magical weapons, immune to most
/// things else" — we keep parity with the rest of the undead pool
/// here).
///
/// Damage profile: resistance to acid / cold / fire / lightning /
/// thunder + non-magical B/P/S; immunity to necrotic and poison.
/// Condition immunity to charm / fright / exhaustion / grapple /
/// paralysis / petrification / poisoned / prone / restrained — the
/// standard incorporeal lockdown.
pub static GHOST_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*GHOST_WITHERING_TOUCH);
    actions.push(&*GHOST_HORRIFYING_VISAGE);
    CreatureTemplate {
        name: "Ghost",
        // 'g' was free (G is Gargoyle / Goblin / Giant — uppercase).
        glyph: 'g',
        ac: 11,
        // 10d8 = 45 average per MM.
        hitpoints: "10d8".parse().unwrap(),
        speed: 30.,
        strength: 7,
        intelligence: 10,
        dexterity: 13,
        wisdom: 12,
        constitution: 10,
        charisma: 17, // drives the Possession DC if ever modeled
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common]),
        cr: 4.0,
        size: Size::Medium,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        damage_modifiers: HashMap::from([
            (DamageType::Necrotic, DamageModifier::Immunity),
            (DamageType::Poison, DamageModifier::Immunity),
            // Resistance to acid / cold / fire / lightning / thunder
            // (incorporeal envelope) + non-magical B/P/S.
            (DamageType::Acid, DamageModifier::Resistance),
            (DamageType::Cold, DamageModifier::Resistance),
            (DamageType::Fire, DamageModifier::Resistance),
            (DamageType::Lightning, DamageModifier::Resistance),
            (DamageType::Thunder, DamageModifier::Resistance),
            (DamageType::Bludgeoning, DamageModifier::Resistance),
            (DamageType::Piercing, DamageModifier::Resistance),
            (DamageType::Slashing, DamageModifier::Resistance),
        ]),
        proficient_saves: HashSet::new(),
        // The 5e ghost condition envelope: immune to a long list because
        // the body is incorporeal and the mind is already dead.
        condition_immunities: HashSet::from([
            Condition::Charmed,
            Condition::Frightened,
            Condition::Exhausted,
            Condition::Grappled,
            Condition::Paralyzed,
            Condition::Petrified,
            Condition::Poisoned,
            Condition::Prone,
            Condition::Restrained,
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
        has_magic_resistance: false,
        recharge_abilities: Vec::new(),
    }
});
