use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{GIANT_ELK_CHARGE, GIANT_ELK_RAM};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size, Skill, SpecialSense,
};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Giant Elk — CR 2 huge **celestial**. Not a bigger elk: a good
/// creature shaped like one, which the stat block says three separate
/// ways.
///
///   - The ram carries **2d4 radiant** alongside its bludgeoning, so
///     the elk hits undead the way a cleric's weapon does.
///   - It **resists necrotic and radiant** — the two damage types a
///     celestial spends its existence in the middle of.
///   - It speaks Celestial and understands three mortal languages, and
///     it is Neutral Good, which is the only alignment on the whole
///     beast shelf that could be an argument.
///
/// Mechanically it sits where a CR-2 charger sits — sixty feet of
/// speed, a reach-10 ram, and a knockdown clause off a twenty-foot run
/// — with the radiant rider making it disproportionately good against
/// exactly the creatures a party would want a celestial's help with.
///
/// Action lane:
/// - **giant elk ram** — STR-based 2d6+STR bludgeoning at reach 2, plus
///   a flat 2d4 radiant rider on every hit, plus the charge rider when
///   it earns one.
///
/// Stat shape: AC 14, ~42 HP (5d12+10), STR 19, DEX 18, CON 14, INT 7,
/// WIS 14, CHA 10. Speed 60. Skills Perception. Saves STR, DEX. Senses
/// Darkvision 90. Size Huge. CR 2. XP 450 per RAW.
pub static GIANT_ELK_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GIANT_ELK_RAM);
    CreatureTemplate {
        name: "Giant Elk",
        // 'E' — the uppercase antlered silhouette above the ordinary
        // elk's lowercase 'e' cohort.
        glyph: 'E',
        ac: 14,
        hitpoints: "5d12+10".parse().unwrap(),
        speed: 60.,
        strength: 19,
        intelligence: 7,
        dexterity: 18,
        wisdom: 14,
        constitution: 14,
        charisma: 10,
        skills: HashSet::from([Skill::Perception]),
        senses: HashSet::from([SpecialSense::Darkvision(90)]),
        languages: HashSet::from([Language::Celestial]),
        cr: 2.0,
        size: Size::Huge,
        creature_type: CreatureType::Celestial,
        actions,
        // RAW **Resistances: Necrotic, Radiant** — the celestial's own
        // element and its opposite.
        damage_modifiers: HashMap::from([
            (DamageType::Necrotic, DamageModifier::Resistance),
            (DamageType::Radiant, DamageModifier::Resistance),
        ]),
        proficient_saves: HashSet::from([AbilityScoreType::Strength, AbilityScoreType::Dexterity]),
        charge: Some(GIANT_ELK_CHARGE),
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
            &GIANT_ELK_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn giant_elk_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 2.0);
        assert_eq!(a.size(), Size::Huge);
        assert!(a.find_action("giant elk ram").is_some());
        assert!(a.charge().is_some());
    }

    /// The three lines that make it a celestial rather than a large
    /// elk, pinned together because dropping any one of them turns the
    /// template into the wrong creature without breaking anything.
    #[test]
    fn the_giant_elk_is_a_celestial_and_fights_like_one() {
        let a = make();
        assert_eq!(a.creature_type(), CreatureType::Celestial);
        assert_eq!(
            a.damage_modifier(DamageType::Necrotic),
            Some(DamageModifier::Resistance)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Radiant),
            Some(DamageModifier::Resistance)
        );
        assert_eq!(
            crate::actions::monster_attacks::GIANT_ELK_RAM.rider_type,
            DamageType::Radiant
        );
    }
}
