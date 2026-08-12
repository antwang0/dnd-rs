use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::SPHINX_OF_WONDER_REND;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{
    CreatureType, DamageModifier, DamageType, Language, Size, Skill, SpecialSense,
};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Sphinx of Wonder — CR 1 tiny celestial, and the bottom rung of a
/// ladder that previously started at the Androsphinx (CR 17).
///
/// It is a house-cat-sized sphinx with wings, and every line on the
/// stat block is defensive except one:
///   - **Magic Resistance** on a 24-hit-point frame, which is an
///     absurd pairing and exactly the point — a save-or-suck aimed at
///     the sphinx rolls at advantage against it, and a greatsword kills
///     it in one swing.
///   - **Resistance to necrotic, psychic and radiant**, the three
///     damage types the things a sphinx guards against tend to use.
///   - and a **rend** whose radiant half is more than twice its
///     physical half.
///
/// The result is a creature that survives spells and dies to soldiers,
/// which is the inverse of nearly everything else at CR 1 and makes it
/// worth fielding beside them rather than instead of them.
///
/// Action lane:
/// - **sphinx rend** — DEX-based 1d4+DEX slashing plus a flat 2d6
///   radiant rider.
///
/// **Burst of Ingenuity** (RAW: a 2/day reaction adding 2 to any
/// ability check or saving throw made within 30 feet, its own or an
/// ally's) is not modeled. The engine's reaction lanes are triggered by
/// events on the board — a move out of reach, an incoming attack — and
/// there is no hook on a save's *roll* for a bystander to interpose a
/// flat bonus into. Worth naming rather than quietly dropping: it is
/// the one clause that makes a sphinx of wonder a support creature
/// rather than a fragile skirmisher.
///
/// Stat shape: AC 13, ~24 HP (7d4+7), STR 6, DEX 17, CON 13, INT 15,
/// WIS 12, CHA 11. Speed 20 walking, fly 40. Skills Arcana, Religion,
/// Stealth. Senses Darkvision 60. Languages Celestial, Common. Size
/// Tiny. CR 1. XP 200 per RAW.
pub static SPHINX_OF_WONDER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&SPHINX_OF_WONDER_REND);
    CreatureTemplate {
        name: "Sphinx of Wonder",
        // 'q' — the small winged cat. The Androsphinx holds the
        // uppercase end of the sphinx family; this is the tiny one.
        glyph: 'q',
        ac: 13,
        hitpoints: "7d4+7".parse().unwrap(),
        speed: 20.,
        fly_speed: 40.,
        strength: 6,
        intelligence: 15,
        dexterity: 17,
        wisdom: 12,
        constitution: 13,
        charisma: 11,
        skills: HashSet::from([Skill::Arcana, Skill::Religion, Skill::Stealth]),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Celestial, Language::Common]),
        cr: 1.0,
        size: Size::Tiny,
        creature_type: CreatureType::Celestial,
        actions,
        damage_modifiers: HashMap::from([
            (DamageType::Necrotic, DamageModifier::Resistance),
            (DamageType::Psychic, DamageModifier::Resistance),
            (DamageType::Radiant, DamageModifier::Resistance),
        ]),
        // The line that makes a 24-HP creature awkward to remove with a
        // spell, and the reason the template is interesting at all.
        has_magic_resistance: true,
        ..CreatureTemplate::defaults()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::Coordinate;

    fn make() -> ActorInstance {
        ActorInstance::from_creature_template(
            &SPHINX_OF_WONDER_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn sphinx_of_wonder_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 1.0);
        assert_eq!(a.size(), Size::Tiny);
        assert_eq!(a.creature_type(), CreatureType::Celestial);
        assert!(a.find_action("sphinx rend").is_some());
    }

    /// Magic Resistance on a frame this small is the whole design, and
    /// the pairing a "surely this is a typo" cleanup would undo.
    #[test]
    fn the_smallest_sphinx_still_shrugs_off_spells() {
        let a = make();
        assert!(a.has_magic_resistance());
        assert!(a.max_hitpoints() < 40, "and it is still made of paper");
    }
}
