use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::GIANT_BAT_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Giant Bat — CR ¼ large beast. The "cave-dweller flier" tier: a
/// blindsight-driven nocturnal hunter with a single bite swing. Slots
/// beside the Hawk (CR 0 tiny flier), Stirge (CR ⅛ flying blood-
/// drainer), and Pteranodon (CR ¼ flying dino) on the low-CR flying-
/// beast bench. Distinguished from the diurnal Hawk / Eagle pool by
/// **Blindsight 60** — the bat doesn't care about darkness or
/// invisibility within its echolocation cone.
///
/// Action lane:
/// - **giant bat bite** — STR-based 1d6+STR piercing melee via the
///   shared `GIANT_BAT_BITE` static. Single swing per Action (no
///   multi); the bat's threat profile is mobility + sensory, not
///   damage. RAW: "Hit: 5 (1d6 + 2) piercing damage."
///
/// **Echolocation** (RAW: the bat can't use its blindsight while
/// deafened) is partially modeled — Blindsight is a senses entry,
/// not a condition-gated trait, so a Deafened bat retains blindsight
/// in this engine (the Deafened condition is mostly cosmetic in
/// this engine anyway). The simplification is small in practice —
/// no creature in the pool installs Deafened on the bat. **Keen
/// Hearing** (RAW: advantage on Perception checks using hearing) is
/// flavor-only — the engine doesn't surface skill checks through
/// combat.
///
/// Defensive identity: AC 13 (large + decent DEX), 22 HP (5d10-5).
/// Vanilla beast envelope — no resistances or condition immunities.
/// The bat dies to two solid hits; its threat lives in the speed-60
/// mobility and blindsight-60 anti-stealth envelope (an invisible
/// rogue inside 60ft of the bat loses the Invisible attacker-
/// disadvantage benefit; see `engine::attack`'s blindsight gate).
///
/// Stat shape: AC 13, ~22 HP (5d10-5), STR 15, DEX 16, CON 11, INT 2,
/// WIS 12, CHA 6. Speed 10 walking + fly 60 (collapsed to fly 60 as
/// the per-creature speed). Senses: Blindsight 60. Size Large. CR ¼.
/// XP: 50 per RAW.
pub static GIANT_BAT_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GIANT_BAT_BITE);
    CreatureTemplate {
        name: "Giant Bat",
        // 'B' (uppercase) — shared with Brown Bear / Bugbear / Bandit
        // Captain cohort; the beast vs humanoid context disambiguates
        // in the prompt and the team color separates them on the map.
        // 'b' (lowercase) is taken by Owlbear / Berserker. Uppercase
        // 'B' reads as "large flier silhouette" at the small UI scale.
        glyph: 'B',
        ac: 13,
        // 5d10-5 = 22.5 average per MM (CR ¼). Floored at 1 for
        // unlucky dice rolls.
        hitpoints: "5d10-5".parse().unwrap(),
        // Fly 60 — the bat's signature mobility, mirroring Hawk /
        // Giant Eagle / Pteranodon. Engine collapses ground + fly to
        // a single per-creature speed since the bat almost never walks.
        // RAW speed line: Speed 10 ft., fly 60 ft.
        speed: 10.0,
        fly_speed: 60.0,
        strength: 15,
        intelligence: 2,
        dexterity: 16,
        wisdom: 12,
        constitution: 11,
        charisma: 6,
        // Blindsight 60 — the bat's signature anti-stealth sense.
        // Echolocation-driven; inside 60ft of the bat, an invisible
        // or hidden target loses the standard attacker-disadvantage
        // gate (the engine routes Blindsight through the concealment
        // chokepoint in `engine::attack`). The load-bearing sensory
        // trait that separates the bat from the Hawk / Eagle pool.
        senses: HashSet::from([SpecialSense::Blindsight(60)]),
        cr: 0.25,
        size: Size::Large,
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
            &GIANT_BAT_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn giant_bat_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.25);
        assert_eq!(a.size(), Size::Large);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("giant bat bite").is_some());
    }

    #[test]
    fn giant_bat_carries_blindsight() {
        // Pin the load-bearing sensory trait: Blindsight 60 is what
        // separates the bat from the Hawk / Eagle pool. A future
        // template refactor that quietly stripped Blindsight would
        // demote the bat to "a slightly larger Hawk" — flattening
        // the cave-echo-locator identity.
        let a = make();
        assert!(a.senses().contains(&SpecialSense::Blindsight(60)));
    }

    #[test]
    fn giant_bat_is_fast_flier() {
        // Pin the mobility trait: fly 60 is the bat's identity.
        // Dropping to the default 30 would erase the swooping-cave-
        // hunter silhouette.
        let a = make();
        assert!(a.speed() >= 60.0);
    }
}
