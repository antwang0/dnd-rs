use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{FLYING_SWORD_SLASH, SLAM};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Animated Armor — CR 1 construct. A walking suit of armor: high AC,
/// modest HP, and the usual construct immunity suite. Slams instead of
/// any natural weapon. Construct immunities make it a natural pairing
/// with charm / sleep / poison spell loadouts — Color Spray, Sleep, and
/// Charm Person all bounce off.
pub static ANIMATED_ARMOR_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&SLAM);
    CreatureTemplate {
        name: "Animated Armor",
        // 'I' for "iron armor" — distinct from 'A' (Aboleth) and 'a'
        // (Amulet of Health ground glyph).
        glyph: 'I',
        ac: 18,
        hitpoints: "6d8+6".parse().unwrap(),
        speed: 25.,
        strength: 14,
        intelligence: 1,
        dexterity: 11,
        wisdom: 3,
        constitution: 13,
        charisma: 1,
        // Blindsight — animated objects "see" without conventional sight.
        senses: HashSet::from([SpecialSense::Blindsight(60)]),
        cr: 1.0,
        size: Size::Medium,
        creature_type: CreatureType::Construct,
        actions,
        // Constructs are immune to poison and psychic damage in 5e.
        damage_modifiers: HashMap::from([
            (DamageType::Poison, DamageModifier::Immunity),
            (DamageType::Psychic, DamageModifier::Immunity),
        ]),
        // Standard construct immunity suite — Asleep is explicitly listed
        // alongside Charmed for documentation clarity (the engine's
        // dynamic_immunity_to chokepoint already gates Asleep on Charmed
        // for several other immunity sources).
        condition_immunities: HashSet::from([
            // SRD 5.2 "Immunities Poison, Psychic; Charmed, Deafened,
            // Exhaustion, Frightened, Paralyzed, Petrified, Poisoned".
            // A construct has no stamina to spend.
            Condition::Exhausted,
            Condition::Poisoned,
            Condition::Charmed,
            Condition::Frightened,
            Condition::Paralyzed,
            Condition::Petrified,
            Condition::Blinded,
            Condition::Deafened,
            Condition::Asleep,
        ]),
        ..CreatureTemplate::defaults()
    }
});

/// Animated Flying Sword — CR ¼ small construct. A blade with nobody
/// holding it.
///
/// The cheapest thing on the construct bench and the only one that
/// flies: AC 17 on fourteen hit points, hovering at fifty feet of
/// speed. It is a glass needle — two solid hits and it is scrap — and
/// the armour class is the whole defence, which makes it the bestiary's
/// clearest lesson in why a party carries something that does not roll
/// to hit.
///
/// Hovers, per RAW's "(hover)". That is load-bearing rather than
/// decorative on a creature with a walking speed of five: without it,
/// the general flying rule would drop the sword to the floor the first
/// time anything knocked it prone or held it still, and a sword on the
/// floor moves one tile a turn.
///
/// Its Slash is keyed to Dexterity, which is unusual for a melee swing
/// and is what the stat block says — there is no arm behind it, so what
/// decides whether it lands is how fast it moves.
pub static FLYING_SWORD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&FLYING_SWORD_SLASH);
    CreatureTemplate {
        name: "Animated Flying Sword",
        // '/' — the blade itself, and the one glyph on the board that
        // looks like the thing it names.
        glyph: '/',
        ac: 17,
        // 4d6 = 14 average per SRD 5.2 (CR ¼).
        hitpoints: "4d6".parse().unwrap(),
        // RAW speed line: Speed 5 ft., Fly 50 ft. (hover). The five feet
        // is the sword dragging its point along the floor.
        speed: 5.,
        fly_speed: 50.,
        hovers: true,
        strength: 12,
        dexterity: 15,
        constitution: 11,
        intelligence: 1,
        wisdom: 5,
        charisma: 1,
        senses: HashSet::from([SpecialSense::Blindsight(60)]),
        cr: 0.25,
        size: Size::Small,
        creature_type: CreatureType::Construct,
        actions,
        damage_modifiers: HashMap::from([
            (DamageType::Poison, DamageModifier::Immunity),
            (DamageType::Psychic, DamageModifier::Immunity),
        ]),
        condition_immunities: HashSet::from([
            Condition::Exhausted,
            Condition::Poisoned,
            Condition::Charmed,
            Condition::Frightened,
            Condition::Paralyzed,
            Condition::Petrified,
            Condition::Blinded,
            Condition::Deafened,
            Condition::Asleep,
        ]),
        ..CreatureTemplate::defaults()
    }
});
