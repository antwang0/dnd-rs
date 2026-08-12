use crate::actions::class_features::MAGICAL_ATTACKS_TAG;
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{PLANETAR_HOLY_BURST, PLANETAR_MULTI, PLANETAR_SWORD};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size, Skill, SpecialSense,
};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Planetar — CR 16 large celestial, and the middle of an angel ladder
/// that ran Deva (CR 10) to Solar (CR 21) with nothing in between.
///
/// It is the first creature on the roster that is a genuine *choice*
/// between two shapes of turn at the top tier:
///   - **three radiant swords** at reach 10, ninety-six average damage
///     into one target, or
///   - **holy burst**, a 7d6 radiant sphere that lands on every enemy
///     within forty feet of a point it can see and leaves its allies
///     alone.
///
/// Nothing else in the bestiary at this tier has a friend-or-foe
/// distinction on its area damage; a dragon's breath does not care whose
/// side anybody is on, and neither does a demon's. An angel's does, and
/// that is what `PointBurstSaveDamage::enemies_only` exists to say.
///
/// Action lanes:
/// - **planetar multiattack** — three radiant swords.
/// - **radiant sword** — the single swing, reach 2 (10 ft).
/// - **holy burst** — the enemies-only radiant sphere.
///
/// Defensive identity: AC 19, 262 hit points, radiant resistance,
/// immunity to Charmed / Exhaustion / Frightened, Magic Resistance,
/// truesight 120, and flight at 120 feet **with hover** — the hover is
/// load-bearing rather than decorative, because it is what keeps a
/// planetar in the air through the general flying rule when something
/// finally lands a stun on it.
///
/// Not modeled: **Divine Awareness**, **Exalted Restoration** and the
/// spell slate (*Commune*, *Control Weather*, *Dispel Evil and Good*,
/// *Raise Dead*, and the 2/day **Divine Aid** bonus action) are all
/// out of an encounter's scope or out of the engine's spell lane for a
/// slotless caster — the same boundary the ice devil's Ice Wall runs
/// into, described there. The Multiattack's *"or uses Holy Burst
/// twice"* is a choice the AI already has between two list entries; the
/// doubling is what is missing, and `PLANETAR_MULTI` says why.
///
/// Stat shape: AC 19, ~262 HP (21d10+147), STR 24, DEX 20, CON 24,
/// INT 19, WIS 22, CHA 25. Speed 40 walking, fly 120 (hover). Skills
/// Perception. Saves STR, CON, WIS, CHA. Senses Truesight 120.
/// Languages Celestial (RAW: all). Size Large. CR 16. XP 15,000 per RAW.
pub static PLANETAR_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&PLANETAR_SWORD);
    actions.push(&*PLANETAR_MULTI);
    actions.push(&PLANETAR_HOLY_BURST);
    CreatureTemplate {
        name: "Planetar",
        // 'V' — free on the uppercase shelf. 'A' is the ape/angel
        // crowd and 'S' the solar's; the planetar takes its own letter
        // so the three angels are three silhouettes.
        glyph: 'V',
        ac: 19,
        hitpoints: "21d10+147".parse().unwrap(),
        speed: 40.,
        fly_speed: 120.,
        // RAW's "(hover)" — and the clause that keeps a planetar aloft
        // when a stun would drop any other flier out of the sky.
        hovers: true,
        strength: 24,
        intelligence: 19,
        dexterity: 20,
        wisdom: 22,
        constitution: 24,
        charisma: 25,
        skills: HashSet::from([Skill::Perception]),
        senses: HashSet::from([SpecialSense::Truesight(120)]),
        languages: HashSet::from([Language::Celestial, Language::Common]),
        cr: 16.0,
        size: Size::Large,
        creature_type: CreatureType::Celestial,
        actions,
        damage_modifiers: HashMap::from([(DamageType::Radiant, DamageModifier::Resistance)]),
        condition_immunities: HashSet::from([Condition::Charmed, Condition::Frightened]),
        proficient_saves: HashSet::from([
            AbilityScoreType::Strength,
            AbilityScoreType::Constitution,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]),
        has_magic_resistance: true,
        features: HashSet::from([MAGICAL_ATTACKS_TAG]),
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
            &PLANETAR_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn planetar_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 16.0);
        assert_eq!(a.size(), Size::Large);
        assert_eq!(a.creature_type(), CreatureType::Celestial);
        assert!(a.find_action("planetar multiattack").is_some());
        assert!(a.find_action("holy burst").is_some());
    }

    /// The hover is the difference between an angel that gets stunned
    /// and an angel that gets stunned *and falls a hundred feet*.
    #[test]
    fn a_stunned_planetar_stays_in_the_air() {
        use crate::conditions::ConditionTimer;
        let mut a = make();
        a.add_condition(Condition::Stunned, ConditionTimer::Rounds(1));
        assert!(
            a.is_airborne(),
            "RAW's (hover) exempts the planetar from the general flying rule"
        );
    }
}
