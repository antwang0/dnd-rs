use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::RAT_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Rat — CR 0 tiny beast. The canonical "lone rodent" ambient vermin:
/// a single bite swing for 1 piercing on a 1-HP frame, with the
/// Darkvision-30 anti-darkness envelope. Slots beside the Bat (CR 0
/// tiny flier), Hawk (CR 0 tiny scout), Cat (CR 0 tiny climber) at the
/// very bottom of the CR ladder. Distinct from the Giant Rat (CR ⅛
/// small + Pack Tactics 1d4 bite) by being smaller, lacking pack
/// tactics, and dealing flat 1 damage instead of 1d4.
///
/// Action lane:
/// - **rat bite** — STR-based flat-1 piercing melee via the shared
///   `RAT_BITE` static. The rat's only swing. The single rat is
///   trivial; the threat profile is "ambient noise creature, the
///   sewer is full of these but they don't matter individually."
///
/// **Keen Smell** (RAW: advantage on Perception checks using smell)
/// is flavor-only — the engine doesn't surface skill checks through
/// combat.
///
/// Defensive identity: AC 10 (tiny + no notable DEX), 1 HP. Vanilla
/// beast envelope — no resistances or condition immunities. The rat
/// dies to any solid hit; flavor-wise it's "the dungeon's ambient
/// vermin," not a meaningful combat threat.
///
/// Stat shape: AC 10, ~1 HP (1d4-1 → floored at 1), STR 2, DEX 11,
/// CON 9, INT 2, WIS 10, CHA 4. Speed 20. Senses: Darkvision 30
/// (sewer-and-attic low-light vision — shorter than the Hawk / Cat
/// 60-ft envelope, matching RAW). Size Tiny. CR 0. XP: 10 per RAW.
pub static RAT_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&RAT_BITE);
    CreatureTemplate {
        name: "Rat",
        // 'r' (lowercase) — small / tiny rodent silhouette, shared with
        // Giant Rat / Dryad. The team color disambiguates on the map
        // and the same-glyph Giant Rat sits one CR tier up (CR ⅛) so
        // the two coexist as "lone rat" vs "swarm rat" silhouettes.
        // Uppercase 'R' is taken by Adult Red Dragon / Ranger / Rogue
        // cohort.
        glyph: 'r',
        ac: 10,
        // RAW: 1 (1d4 - 1) — engine floors HP rolls at 1.
        hitpoints: "1d4-1".parse().unwrap(),
        speed: 20.,
        strength: 2,
        intelligence: 2,
        dexterity: 11,
        wisdom: 10,
        constitution: 9,
        charisma: 4,
        // Darkvision 30 — the rat's signature low-light sense. Shorter
        // than the standard 60-ft envelope of most nocturnal beasts
        // (matching RAW's tiny-rodent stat block); routes through the
        // same `SpecialSense::Darkvision` chokepoint.
        senses: HashSet::from([SpecialSense::Darkvision(30)]),
        cr: 0.0,
        size: Size::Tiny,
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
            &RAT_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn rat_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.0);
        assert_eq!(a.size(), Size::Tiny);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("rat bite").is_some());
    }

    #[test]
    fn rat_lacks_pack_tactics() {
        // Pin the load-bearing scaling distinction: the regular rat is
        // a *lonely* rodent — RAW doesn't grant it Pack Tactics. A
        // future template refactor that quietly toggled `has_pack_
        // tactics: true` would collapse the "lone vermin" tier into
        // the Giant Rat's "swarm vermin" niche, erasing the load-
        // bearing tactical distinction between the two adjacent CR
        // tiers.
        let a = make();
        assert!(!a.has_pack_tactics());
    }
}
