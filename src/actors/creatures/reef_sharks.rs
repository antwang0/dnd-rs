use crate::actions::class_features::SWIM_SPEED_TAG;
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::REEF_SHARK_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Reef Shark — CR ½ medium beast. The smallest entry on the shark
/// ladder: a 5-foot reef predator that hunts in packs along the
/// coastline. Slots below the Hunter Shark (CR 2 large solo
/// frenzy-biter) and the Giant Shark (CR 5 huge apex frenzy-biter)
/// at the low end of the shark family. The defining trait is
/// **Pack Tactics**: every reef shark bite rolls at advantage when
/// an ally shark is adjacent to the target — the classic "swarm in
/// the surf" multiplier the cohort rides on.
///
/// Action lane:
/// - **reef shark bite** — STR-based 1d8+STR piercing melee via the
///   shared `REEF_SHARK_BITE` static. Vanilla SimpleWeapon — no
///   rider. The threat multiplier lives on the template-level
///   `has_pack_tactics: true` flag, not the bite itself: three
///   reef sharks circling the same swimmer roll every bite at
///   advantage and chew through the target's HP fast.
///
/// **Water Breathing** (RAW: can breathe only underwater) is
/// flavor-only — the engine doesn't model the aquatic-vs-land
/// terrain split, so the clause collapses to the per-creature
/// 40 walking speed (RAW: swim 40, no land speed; we use the
/// swim number as the per-creature speed since reef sharks never
/// walk).
///
/// Defensive identity: AC 12 (medium + 13 DEX), 22 HP (4d8+4).
/// Vanilla beast envelope — no resistances or condition
/// immunities. The reef shark dies to two solid hits; its threat
/// lives in the pack-tactics-amplified bite. **Blindsight 30**
/// lets the reef shark hunt by water-pressure sensing through
/// the engine's invisibility / illusion concealment chokepoint
/// at short range — the same sensory envelope the Spider /
/// Giant Wolf Spider cohort uses for web-vibration awareness.
///
/// Stat shape: AC 12, ~22 HP (4d8+4), STR 14, DEX 13, CON 13,
/// INT 1, WIS 10, CHA 4. Speed 40 (RAW swim 40; we collapse to
/// the swim number as per-creature speed). Senses: Blindsight 30.
/// Size Medium. CR ½. XP: 100 per RAW.
pub static REEF_SHARK_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&REEF_SHARK_BITE);
    CreatureTemplate {
        name: "Reef Shark",
        // 'q' (lowercase) — small shark silhouette. 'S' is Sahuagin /
        // Stone Giant / Storm Giant; 'q' is free and reads as
        // "small finned predator" at the small UI scale. The shark
        // family shares the lowercase 'q' (reef) → uppercase 'Q'
        // (hunter) → larger uppercase 'Q' (giant) ladder so all
        // three sharks read as the same silhouette at progressively
        // larger sizes.
        glyph: 'q',
        ac: 12,
        // 4d8+4 = 22 average per MM (CR ½).
        hitpoints: "4d8+4".parse().unwrap(),
        speed: 40.,
        strength: 14,
        intelligence: 1,
        dexterity: 13,
        wisdom: 10,
        constitution: 13,
        charisma: 4,
        senses: HashSet::from([SpecialSense::Blindsight(30)]),
        cr: 0.5,
        size: Size::Medium,
        creature_type: CreatureType::Beast,
        actions,
        has_pack_tactics: true,
        // RAW swim speed: the tag is what makes `TerrainType::Water`
        // free to cross and lifts the underwater melee penalty.
        features: HashSet::from([SWIM_SPEED_TAG]),
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
            &REEF_SHARK_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn reef_shark_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.5);
        assert_eq!(a.size(), Size::Medium);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("reef shark bite").is_some());
    }

    #[test]
    fn reef_shark_has_pack_tactics() {
        // Pin the load-bearing combat trait: Pack Tactics is what
        // separates the reef shark from a smaller hunter shark — the
        // packs that hunt the coastline are dangerous in numbers, not
        // individually. A future template-refactor that quietly
        // stripped Pack Tactics would erase the "swarm in the surf"
        // identity and silently demote the reef shark to "a smaller,
        // weaker hunter shark."
        let a = make();
        assert!(a.has_pack_tactics());
    }
}
