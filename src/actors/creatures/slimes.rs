use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::ACID_SPIT;
use crate::actors::actor_template::{CreatureTemplate, damage_modifiers_from};
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Size};
use std::sync::LazyLock;

/// Acid Slime — squishy ranged splash dealer. Spits an acidic blob at a
/// single target; the impact splashes onto every combat-active actor
/// footprint-adjacent to the primary, so positioning matters around it
/// (don't bunch up downwind of an enemy near a slime, and don't park your
/// slime next to your own front line).
///
/// Stats are rough — light HP, low AC, no melee. Slimes want range.
pub static SLIME_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*ACID_SPIT);
    CreatureTemplate {
        name: "Slime",
        glyph: 's',
        ac: 10,
        hitpoints: "2d6+2".parse().unwrap(),
        speed: 20.,
        strength: 8,
        dexterity: 12,
        constitution: 12,
        intelligence: 2,
        wisdom: 6,
        charisma: 1,
        cr: 0.25,
        size: Size::Small,
        creature_type: CreatureType::Ooze,
        actions,
        // Acid slimes: immune to their own element. Resist BPS — physical
        // weapons gum up in the ooze. Cold turns the gel hard and brittle.
        damage_modifiers: damage_modifiers_from([
            (DamageType::Acid, DamageModifier::Immunity),
            (DamageType::Cold, DamageModifier::Vulnerability),
        ]),
        ..CreatureTemplate::resistant_to_nonmagical_physical()
    }
});
