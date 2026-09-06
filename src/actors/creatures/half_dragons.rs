//! **Half-Dragon** — CR 5, and the one stat block in SRD 5.2 that is
//! printed as five.
//!
//! RAW's Draconic Origin trait reads: "The half-dragon is related to a
//! type of dragon associated with one of the following damage types
//! (GM's choice): Acid, Cold, Fire, Lightning, or Poison. **This choice
//! affects other aspects of the stat block.**" Three lines read it —
//! the resistance, the claw's rider and the breath — and every one of
//! them is a different creature depending on the answer.
//!
//! A GM's choice is not a thing a bestiary can defer, so the choice is
//! made five times and the five are the entries. That is the same call
//! `dragons.rs` makes for a colour, and it is made the same way: one
//! row per variant, one function that builds a template from a row, and
//! the arrays split apart only because a `Multiattack` needs to borrow
//! its sub-attack as `&'static dyn Action` and a `const` cannot be
//! borrowed.
//!
//! What the half-dragon is *for* is the rung between a wyrmling and a
//! young dragon. It flies nowhere, it has no legendary anything, and it
//! is a humanoid-shaped thing with 105 hit points, a breath weapon and
//! AC 18 — which is to say it is the fight a party gets when the dragon
//! they were promised sends somebody instead.
//!
//! Not modelled: **Leap**, RAW's bonus action to jump thirty feet for
//! ten feet of movement. The board has no third axis and no gaps to
//! clear, so a jump is a walk that costs less, and the engine has no
//! lane for buying movement at a discount that would not simply read as
//! a worse Dash.

use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{BreathWeapon, Multiattack, WeaponWithRider};
use crate::actors::actor_template::{CreatureTemplate, damage_modifiers_from};
use crate::engine::dice::Dice;
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size, Skill, SpecialSense,
};
use std::collections::HashSet;
use std::sync::LazyLock;

/// The five names, in the order RAW lists the damage types.
const HALF_DRAGON_NAMES: [&str; 5] = [
    "Acid Half-Dragon",
    "Cold Half-Dragon",
    "Fire Half-Dragon",
    "Lightning Half-Dragon",
    "Poison Half-Dragon",
];

/// One glyph for all five, because on a board they *are* one creature:
/// the element is a resistance and a damage type, not a silhouette, and
/// a party that needs to know which one it is finds out by hitting it.
/// Lowercase against the dragons' own uppercase band, which is the
/// right read — it is the smaller thing the dragon sent.
const HALF_DRAGON_GLYPH: char = 'h';

/// The damage type each variant's Draconic Origin names, in the same
/// order. Read by the claw rider, the breath and the resistance, which
/// is exactly the "affects other aspects of the stat block" clause.
const HALF_DRAGON_ELEMENTS: [DamageType; 5] = [
    DamageType::Acid,
    DamageType::Cold,
    DamageType::Fire,
    DamageType::Lightning,
    DamageType::Poison,
];

/// One claw per variant. RAW: "+7, reach 10 ft. Hit: 6 (1d4 + 4)
/// Slashing damage plus 7 (2d6) damage of the type chosen for the
/// Draconic Origin trait."
///
/// The rider is larger than the swing it rides on, which is the whole
/// character of the creature: its claws are not what hurt, its
/// *bloodline* is.
static HALF_DRAGON_CLAWS: [WeaponWithRider; 5] = [
    WeaponWithRider::reach_melee(
        "acid claw",
        &["hd-claw"],
        AbilityScoreType::Strength,
        Dice::new(1, 4),
        DamageType::Slashing,
        2,
        Dice::new(2, 6),
        DamageType::Acid,
        "draconic blood",
    ),
    WeaponWithRider::reach_melee(
        "cold claw",
        &["hd-claw"],
        AbilityScoreType::Strength,
        Dice::new(1, 4),
        DamageType::Slashing,
        2,
        Dice::new(2, 6),
        DamageType::Cold,
        "draconic blood",
    ),
    WeaponWithRider::reach_melee(
        "fire claw",
        &["hd-claw"],
        AbilityScoreType::Strength,
        Dice::new(1, 4),
        DamageType::Slashing,
        2,
        Dice::new(2, 6),
        DamageType::Fire,
        "draconic blood",
    ),
    WeaponWithRider::reach_melee(
        "lightning claw",
        &["hd-claw"],
        AbilityScoreType::Strength,
        Dice::new(1, 4),
        DamageType::Slashing,
        2,
        Dice::new(2, 6),
        DamageType::Lightning,
        "draconic blood",
    ),
    WeaponWithRider::reach_melee(
        "poison claw",
        &["hd-claw"],
        AbilityScoreType::Strength,
        Dice::new(1, 4),
        DamageType::Slashing,
        2,
        Dice::new(2, 6),
        DamageType::Poison,
        "draconic blood",
    ),
];

/// Two claws per Action, in `HALF_DRAGON_NAMES` order.
static HALF_DRAGON_MULTIATTACKS: [Multiattack; 5] = [
    Multiattack {
        display_name: "double claw",
        sub_attack: &HALF_DRAGON_CLAWS[0],
        count: 2,
    },
    Multiattack {
        display_name: "double claw",
        sub_attack: &HALF_DRAGON_CLAWS[1],
        count: 2,
    },
    Multiattack {
        display_name: "double claw",
        sub_attack: &HALF_DRAGON_CLAWS[2],
        count: 2,
    },
    Multiattack {
        display_name: "double claw",
        sub_attack: &HALF_DRAGON_CLAWS[3],
        count: 2,
    },
    Multiattack {
        display_name: "double claw",
        sub_attack: &HALF_DRAGON_CLAWS[4],
        count: 2,
    },
];

/// **Dragon's Breath** (Recharge 5–6) — DEX DC 14, 8d6 of the origin's
/// type in a 30-foot cone, half on a save.
///
/// Thirty feet is radius 2 on the same scale `dragons.rs` collapses its
/// cones onto, and the pool is the shared `"breath_weapon"` one, so a
/// half-dragon standing beside an actual dragon cannot borrow its
/// recharge.
static HALF_DRAGON_BREATHS: [BreathWeapon; 5] = [
    BreathWeapon {
        display_name: "acid breath",
        aliases: &["breath", "br"],
        damage: Some((Dice::new(8, 6), DamageType::Acid)),
        save_ability: AbilityScoreType::Dexterity,
        dc: 14,
        radius: 2,
        range: 3,
        recharge_key: "breath_weapon",
        condition: None,
        enemies_only: false,
    },
    BreathWeapon {
        display_name: "cold breath",
        aliases: &["breath", "br"],
        damage: Some((Dice::new(8, 6), DamageType::Cold)),
        save_ability: AbilityScoreType::Dexterity,
        dc: 14,
        radius: 2,
        range: 3,
        recharge_key: "breath_weapon",
        condition: None,
        enemies_only: false,
    },
    BreathWeapon {
        display_name: "fire breath",
        aliases: &["breath", "br"],
        damage: Some((Dice::new(8, 6), DamageType::Fire)),
        save_ability: AbilityScoreType::Dexterity,
        dc: 14,
        radius: 2,
        range: 3,
        recharge_key: "breath_weapon",
        condition: None,
        enemies_only: false,
    },
    BreathWeapon {
        display_name: "lightning breath",
        aliases: &["breath", "br"],
        damage: Some((Dice::new(8, 6), DamageType::Lightning)),
        save_ability: AbilityScoreType::Dexterity,
        dc: 14,
        radius: 2,
        range: 3,
        recharge_key: "breath_weapon",
        condition: None,
        enemies_only: false,
    },
    BreathWeapon {
        display_name: "poison breath",
        aliases: &["breath", "br"],
        damage: Some((Dice::new(8, 6), DamageType::Poison)),
        save_ability: AbilityScoreType::Dexterity,
        dc: 14,
        radius: 2,
        range: 3,
        recharge_key: "breath_weapon",
        condition: None,
        enemies_only: false,
    },
];

/// Build the template for the `index`-th Draconic Origin.
///
/// Everything that is not the element is written once here, which is
/// the point: the five stat blocks differ in three lines and RAW says
/// so out loud, so five hand-written literals would be five chances to
/// give the cold one a fire breath.
fn half_dragon_template(index: usize) -> CreatureTemplate {
    let element = HALF_DRAGON_ELEMENTS[index];
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&HALF_DRAGON_MULTIATTACKS[index]);
    actions.push(&HALF_DRAGON_CLAWS[index]);
    actions.push(&HALF_DRAGON_BREATHS[index]);
    CreatureTemplate {
        name: HALF_DRAGON_NAMES[index],
        glyph: HALF_DRAGON_GLYPH,
        ac: 18,
        // 14d8+42 = 105 average per SRD 5.2 (CR 5).
        hitpoints: "14d8+42".parse().expect("half-dragon hit dice parse"),
        speed: 40.,
        strength: 19,
        dexterity: 14,
        constitution: 16,
        intelligence: 10,
        wisdom: 15,
        charisma: 14,
        senses: HashSet::from([SpecialSense::Blindsight(10), SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common, Language::Draconic]),
        cr: 5.0,
        size: Size::Medium,
        // A **Dragon**, not a Humanoid — which is what makes the
        // half-dragon worth a stat block rather than a template, since
        // everything in the game that hunts dragons finds this one.
        creature_type: CreatureType::Dragon,
        actions,
        skills: HashSet::from([Skill::Athletics, Skill::Perception, Skill::Stealth]),
        // The Draconic Origin resistance, third of the three lines the
        // choice reaches.
        //
        // Resistance and not immunity, which is the difference between
        // this and a green dragon: the poison variant takes half from
        // poison damage and can still be Poisoned, because a resistance
        // is not the "immune to poison damage, therefore immune to the
        // condition" sentence the dragons read. No condition immunities
        // at all — RAW prints none.
        damage_modifiers: damage_modifiers_from([(element, DamageModifier::Resistance)]),
        // RAW: DEX +5 and WIS +5 against a +3 proficiency bonus.
        proficient_saves: HashSet::from([AbilityScoreType::Dexterity, AbilityScoreType::Wisdom]),
        recharge_abilities: vec![("breath_weapon", 5)],
        ..CreatureTemplate::defaults()
    }
}

pub static ACID_HALF_DRAGON_TEMPLATE: LazyLock<CreatureTemplate> =
    LazyLock::new(|| half_dragon_template(0));
pub static COLD_HALF_DRAGON_TEMPLATE: LazyLock<CreatureTemplate> =
    LazyLock::new(|| half_dragon_template(1));
pub static FIRE_HALF_DRAGON_TEMPLATE: LazyLock<CreatureTemplate> =
    LazyLock::new(|| half_dragon_template(2));
pub static LIGHTNING_HALF_DRAGON_TEMPLATE: LazyLock<CreatureTemplate> =
    LazyLock::new(|| half_dragon_template(3));
pub static POISON_HALF_DRAGON_TEMPLATE: LazyLock<CreatureTemplate> =
    LazyLock::new(|| half_dragon_template(4));

/// Every half-dragon, for `EncounterInstance::template_pool` and for
/// the sweeps that want to assert something about all five at once.
pub fn all_half_dragon_templates() -> [&'static CreatureTemplate; 5] {
    [
        &ACID_HALF_DRAGON_TEMPLATE,
        &COLD_HALF_DRAGON_TEMPLATE,
        &FIRE_HALF_DRAGON_TEMPLATE,
        &LIGHTNING_HALF_DRAGON_TEMPLATE,
        &POISON_HALF_DRAGON_TEMPLATE,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::action_template::Action;
    use crate::actors::actor_template::ActorInstance;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::Coordinate;

    fn make(t: &'static CreatureTemplate) -> ActorInstance {
        ActorInstance::from_creature_template(
            t,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    /// One origin, three lines, and they agree.
    ///
    /// This is the invariant the tables exist to make true, and it is
    /// the same one `dragons.rs` pins for its forty: RAW says the
    /// Draconic Origin choice "affects other aspects of the stat
    /// block", and the three aspects are the claw's rider, the breath
    /// and the resistance. Written as five hand-rolled literals it
    /// would be five chances to give the cold one a fire breath, and
    /// nothing would catch it.
    #[test]
    fn a_half_dragons_claw_its_breath_and_its_resistance_are_one_element() {
        for (i, element) in HALF_DRAGON_ELEMENTS.into_iter().enumerate() {
            assert_eq!(
                HALF_DRAGON_CLAWS[i].rider_type, element,
                "{}'s claw carries the wrong element",
                HALF_DRAGON_NAMES[i]
            );
            assert_eq!(
                HALF_DRAGON_BREATHS[i].damage.map(|(_, dt)| dt),
                Some(element),
                "{} breathes the wrong element",
                HALF_DRAGON_NAMES[i]
            );
            let a = make(all_half_dragon_templates()[i]);
            assert!(
                a.is_resistant_to(element),
                "{} should resist its own element",
                HALF_DRAGON_NAMES[i]
            );
        }
    }

    /// Every variant carries the whole kit, and the whole kit is three
    /// entries: the pair of claws, the single claw, and the breath.
    #[test]
    fn every_half_dragon_carries_the_same_three_lanes() {
        for (i, t) in all_half_dragon_templates().into_iter().enumerate() {
            let a = make(t);
            assert_eq!(a.cr(), 5.0);
            assert_eq!(a.creature_type(), CreatureType::Dragon);
            assert!(a.find_action("double claw").is_some(), "{}", t.name);
            assert!(
                a.find_action(HALF_DRAGON_CLAWS[i].name()).is_some(),
                "{}",
                t.name
            );
            assert!(
                a.find_action(HALF_DRAGON_BREATHS[i].name()).is_some(),
                "{}",
                t.name
            );
        }
    }
}
