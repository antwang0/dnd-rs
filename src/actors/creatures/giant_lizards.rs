use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::GIANT_LIZARD_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size};
use std::sync::LazyLock;

/// Giant Lizard — CR ¼ large beast. The "underground reptile" pack
/// fodder of the Underdark / swamp biomes: vanilla 1d8+STR bite, AC
/// 12, 19 HP. Slots beside the Giant Frog (CR ¼ medium ambusher), the
/// Mastiff (CR ⅛ medium pup), and the Boar (CR ¼ medium charger) on
/// the low-CR beast bench — the "common dungeon-creature" filler with
/// no riders, just dice. Frequently ridden as a goblin / kobold mount
/// in the published modules; pairs naturally with the kobold pack at
/// the same CR tier.
///
/// Action lane:
/// - **giant lizard bite** — STR-based 1d8+STR piercing melee via the
///   shared `GIANT_LIZARD_BITE` static. RAW: "Melee Weapon Attack: +4
///   to hit, reach 5 ft, one target. Hit: 6 (1d8 + 2) piercing
///   damage." Single swing per Action (no multiattack); the lizard is
///   the per-round-low-damage / large-frame entry on the CR-¼ beast
///   bench vs the giant frog's auto-grapple bite or the mastiff's
///   trip-bite envelope.
///
/// **Spider Climb** (RAW: can climb difficult surfaces, including
/// upside down on ceilings) and **Hold Breath** (RAW: can hold breath
/// for 15 minutes) are flavor-only — the engine doesn't surface 3D
/// movement or aquatic-suffocation mechanics, so both clauses
/// collapse to the per-creature 30 walking speed.
///
/// Defensive identity: AC 12 (large + 13 DEX), 19 HP (3d10+3).
/// Vanilla beast envelope — no resistances or condition immunities.
/// The lizard dies to two solid hits; its threat lives in the
/// 19-HP frame at CR ¼ — it absorbs more than its damage output
/// suggests, which makes it a credible meat-shield in a kobold
/// pack ambush.
///
/// Stat shape: AC 12, ~19 HP (3d10+3), STR 15, DEX 12, CON 13, INT 2,
/// WIS 10, CHA 5. Speed 40. Size Large. CR ¼. XP: 50 per RAW.
pub static GIANT_LIZARD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GIANT_LIZARD_BITE);
    CreatureTemplate {
        name: "Giant Lizard",
        // 'L' (uppercase) — shared with Lizardfolk ('L' too); the
        // beast vs humanoid context disambiguates in the prompt and
        // the team color separates them on the map. 'l' (lowercase)
        // is free but harder to read at small UI scale; uppercase
        // 'L' matches the "large reptile silhouette" the lizardfolk
        // cohort already uses.
        glyph: 'L',
        ac: 12,
        // 3d10+3 = 19 average per MM (CR ¼).
        hitpoints: "3d10+3".parse().unwrap(),
        speed: 40.,
        strength: 15,
        intelligence: 2,
        dexterity: 12,
        wisdom: 10,
        constitution: 13,
        charisma: 5,
        cr: 0.25,
        size: Size::Large,
        // 5e Mounted Combat: MM: "lizardfolk... use giant lizards as mounts and beasts of burden".
        mountable: true,
        creature_type: CreatureType::Beast,
        actions,
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
            &GIANT_LIZARD_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn giant_lizard_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.25);
        assert_eq!(a.size(), Size::Large);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("giant lizard bite").is_some());
    }
}
