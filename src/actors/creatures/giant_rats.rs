use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::GIANT_RAT_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Giant Rat — CR ⅛ small beast. The "sewer swarm" ambient vermin: low
/// HP, low damage, but **Pack Tactics** turns adjacent ally rats into
/// advantage on the bite roll. Slots beside the Stirge (CR ⅛, blood-
/// drain) and the Hyena (CR 0, pack-tactics medium beast) as the
/// cheapest pack-tactics swarmer in the pool. A lone giant rat is
/// trivial; a four-rat ambush in a corridor is genuinely dangerous to
/// a low-level party.
///
/// Action lane:
/// - **giant rat bite** — STR-based 1d4+STR piercing melee via the
///   shared `GIANT_RAT_BITE` static. Lowest dice tier in the monster
///   pool (1d4 — same as a Dagger). Combined with Pack Tactics, every
///   adjacent-ally bite rolls at advantage — the swarm is the threat,
///   not the individual jaw.
///
/// **Pack Tactics** — RAW: "The rat has advantage on an attack roll
/// against a creature if at least one of the rat's allies is within
/// 5 ft of the creature and the ally isn't incapacitated." Routes
/// through the shared `has_pack_tactics: true` template flag — same
/// chokepoint as the Wolf / Kobold / Thug / Tribal Warrior pack lanes.
/// At the giant rat's CR tier the trait is more impactful than at
/// higher tiers: doubled-bite damage on advantage rolls turns a 1d4 +
/// STR mod swing into a credible threat against soft-AC targets.
///
/// Defensive identity: AC 13 (small + DEX), 7 HP (2d6). Vanilla beast
/// envelope — no resistances or condition immunities. The rat dies to
/// a single solid hit; the threat lives in the swarm. **Keen Smell**
/// (advantage on Perception checks using smell) is RAW flavor-only —
/// the engine doesn't surface skill checks through combat.
///
/// Stat shape: AC 13, ~7 HP (2d6), STR 7, DEX 16, CON 11, INT 2,
/// WIS 10, CHA 4. Speed 30. Senses: Darkvision 60. Size Small. CR ⅛.
/// XP: 25 per RAW.
pub static GIANT_RAT_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GIANT_RAT_BITE);
    CreatureTemplate {
        name: "Giant Rat",
        // 'r' (lowercase) — small rodent silhouette. Shared with Dryad
        // ('r'); the team color disambiguates on the map and the beast
        // / fey CR contexts rarely collide in random encounter rolls.
        // 'R' is taken by Adult Red Dragon / Ranger / Rogue cohort.
        glyph: 'r',
        ac: 13,
        // 2d6 = 7 average per MM (CR ⅛).
        hitpoints: "2d6".parse().unwrap(),
        speed: 30.,
        strength: 7,
        intelligence: 2,
        dexterity: 16,
        wisdom: 10,
        constitution: 11,
        charisma: 4,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::new(),
        cr: 0.125,
        size: Size::Small,
        creature_type: CreatureType::Beast,
        actions,
        // 5e Pack Tactics — advantage when an ally is adjacent to the
        // target. The load-bearing trait at the giant rat's CR tier.
        has_pack_tactics: true,
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
            &GIANT_RAT_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn giant_rat_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.125);
        assert_eq!(a.size(), Size::Small);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("giant rat bite").is_some());
    }

    #[test]
    fn giant_rat_carries_pack_tactics() {
        // Pin the load-bearing trait: a lone giant rat is trivial;
        // Pack Tactics is what makes the swarm a credible low-level
        // threat. Stripping the flag would silently demote the rat to
        // "a smaller hyena", flattening its dungeon-vermin identity.
        let a = make();
        assert!(a.has_pack_tactics());
    }
}
