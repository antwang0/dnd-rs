use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::class_features::UNDEAD_FORTITUDE_TAG;
use crate::actions::monster_attacks::{OGRE_ZOMBIE_SLAM, TRIP, ZOMBIE_MULTISLAM};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

pub static ZOMBIE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    // Multislam is the zombie's main attack (2 swings per Action). Trip is
    // an alternative single attack that on hit forces a STR save or prone —
    // less raw damage but disables movement.
    actions.push(&*ZOMBIE_MULTISLAM);
    actions.push(&*TRIP);
    CreatureTemplate {
        name: "Zombie",
        glyph: 'Z',
        ac: 8,
        hitpoints: "2d8+6".parse().unwrap(),
        speed: 20.,
        strength: 13,
        dexterity: 6,
        constitution: 16,
        intelligence: 3,
        wisdom: 6,
        charisma: 5,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common]), // plus one other
        cr: 0.25,
        size: Size::Medium,
        creature_type: CreatureType::Undead,
        actions,
        // Zombies: undead — immune to poison; resistant to necrotic
        // (negative energy is what animates them, so it heals more than
        // it harms). Vulnerable to radiant (turn-undead flavor).
        damage_modifiers: HashMap::from([
            (DamageType::Poison, DamageModifier::Immunity),
            (DamageType::Necrotic, DamageModifier::Resistance),
            (DamageType::Radiant, DamageModifier::Vulnerability),
        ]),
        // Undead: immune to Poisoned and Charmed.
        condition_immunities: HashSet::from([
            // SRD 5.2 "Immunities Poison; Exhaustion, Poisoned" — a corpse walks until
            // it is knocked apart, and never slower.
            Condition::Charmed,
            Condition::Exhausted,
            Condition::Poisoned,
        ]),
        // RAW **Undead Fortitude**, and the trait a zombie is played
        // for: a blow that would put it down instead buys a CON save
        // at DC 5 + the damage, and a zombie that passes stands back up
        // at 1 HP. See `EncounterInstance::try_undead_fortitude`.
        features: HashSet::from([UNDEAD_FORTITUDE_TAG]),
        ..CreatureTemplate::defaults()
    }
});

/// Ogre Zombie — CR 2 large undead. An ogre that has stopped noticing.
///
/// The stat block is the two halves in plain sight: the ogre's body —
/// eighty-five hit points, STR 19, and a slam that rolls 2d8 — behind
/// the zombie's AC 8 and Dexterity 6. It hits about as hard as the
/// living ogre and is twice as hard to kill, and it will never once
/// dodge.
///
/// One swing rather than the ordinary zombie's two, which is the trade
/// 5.2 makes at this rung: an ogre zombie is not a faster zombie, it is
/// a bigger one, and the single 2d8 slam beats the pair of smaller ones
/// on average while giving the party fewer chances to be hit at all.
///
/// **Undead Fortitude** rides here as it does on the zombie above, and
/// on eighty-five hit points it is a different creature: the blow that
/// finally drops an ogre zombie is usually a large one, which is
/// exactly the DC the save is priced at. Killing it with a dagger is
/// the hard way and RAW knows it.
pub static OGRE_ZOMBIE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&OGRE_ZOMBIE_SLAM);
    CreatureTemplate {
        name: "Ogre Zombie",
        // 'z' (lowercase) beside the zombie's 'Z' — the same corpse one
        // size up, which is what it is.
        glyph: 'z',
        ac: 8,
        // 9d10+36 = 85 average per SRD 5.2 (CR 2).
        hitpoints: "9d10+36".parse().unwrap(),
        speed: 30.,
        strength: 19,
        dexterity: 6,
        constitution: 18,
        intelligence: 3,
        wisdom: 6,
        charisma: 5,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common, Language::Giant]),
        cr: 2.0,
        size: Size::Large,
        creature_type: CreatureType::Undead,
        actions,
        damage_modifiers: HashMap::from([
            (DamageType::Poison, DamageModifier::Immunity),
            (DamageType::Necrotic, DamageModifier::Resistance),
            (DamageType::Radiant, DamageModifier::Vulnerability),
        ]),
        condition_immunities: HashSet::from([
            Condition::Charmed,
            Condition::Exhausted,
            Condition::Poisoned,
        ]),
        features: HashSet::from([UNDEAD_FORTITUDE_TAG]),
        ..CreatureTemplate::defaults()
    }
});
