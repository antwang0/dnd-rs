use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::MIMIC_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Mimic — CR 2 shape-shifting monstrosity. Disguises itself as inert
/// scenery, then opens up with an adhesive bite that sticks the victim
/// in place. We don't model the disguise (no perception system yet),
/// but the Adhesive bite rider applies the `Adhered` condition for two
/// rounds — long enough to make the encounter feel like a trap, short
/// enough that a single victim won't be perma-locked. Immune to
/// acid (its own goo doesn't hurt it) and to prone (already amorphous).
pub static MIMIC_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*MIMIC_BITE);
    CreatureTemplate {
        name: "Mimic",
        // 'm' (lowercase) — distinct from 'M' (Mage / Wizard).
        glyph: 'm',
        ac: 12,
        hitpoints: "9d8+18".parse().unwrap(),
        speed: 20.,
        strength: 17,
        dexterity: 12,
        constitution: 15,
        intelligence: 5,
        wisdom: 13,
        charisma: 8,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        cr: 2.0,
        size: Size::Medium,
        creature_type: CreatureType::Monstrosity,
        actions,
        damage_modifiers: HashMap::from([(DamageType::Acid, DamageModifier::Immunity)]),
        // Amorphous: can't be knocked prone.
        condition_immunities: HashSet::from([Condition::Prone]),
        ..CreatureTemplate::defaults()
    }
});
