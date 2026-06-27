use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::GIANT_WOLF_SPIDER_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense, Skill};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Giant Wolf Spider — CR ¼ medium beast. The "burrowing hunter"
/// cousin of the Giant Spider (CR 1): smaller frame, smaller dice,
/// no web — instead a stalker that ambushes from a ground burrow.
/// Slots between the regular Spider (CR 1 medium ambusher with
/// 2d4 venom) and the Giant Frog (CR ¼ auto-grapple bite) on the
/// low-CR creepy-crawly bench. The "lone hunter" silhouette pairs
/// well with the Giant Rat / Hyena pack — a single wolf spider
/// ambushes from the ceiling while a swarm pours in from the floor.
///
/// Action lane:
/// - **giant wolf spider bite** — STR-based 1d6+STR piercing melee
///   with a DC 11 CON save-or-2d6-poison rider via the shared
///   `WeaponWithSaveDamage` chassis (`GIANT_WOLF_SPIDER_BITE`). Same
///   shape as the regular Spider bite but with lighter base dice
///   (1d6 vs 1d10) and a smaller poison die (2d6 vs 2d4 — yes the
///   wolf spider's venom is actually slightly heavier than the
///   giant spider's, RAW; the giant spider compensates with the
///   bigger base hit). The save-rider envelope is the load-bearing
///   threat — a failed CON 11 save against the venom can drop a
///   wounded low-HP target outright.
///
/// **Spider Climb** (RAW: can climb difficult surfaces, including
/// upside down on ceilings) and **Web Sense / Web Walker** (RAW:
/// senses any creature in contact with the same web, ignores
/// movement restrictions of webbing) are all flavor-only — the
/// engine doesn't surface 3D movement or surface-attached webbing.
/// The clauses collapse to the per-creature 40 walking speed.
///
/// Defensive identity: AC 13 (medium + 13 DEX), 11 HP (2d8+2).
/// Vanilla beast envelope — no resistances or condition immunities
/// (the regular Spider gains poison-immunity at CR 1, but the wolf
/// spider deliberately doesn't — it's the smaller, fragiler
/// sibling). The wolf spider dies to a single solid hit; its
/// threat lives in the save-or-poison-damage rider.
///
/// Stat shape: AC 13, ~11 HP (2d8+2), STR 12, DEX 16, CON 13, INT 3,
/// WIS 12, CHA 4. Speed 40. Skills: Perception, Stealth (RAW skill
/// proficiency for the burrow-ambush flavor). Senses: Blindsight
/// 10, Darkvision 60 — the standard arachnid sensory envelope
/// shared with the regular Spider. Size Medium. CR ¼. XP: 50 per
/// RAW.
pub static GIANT_WOLF_SPIDER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GIANT_WOLF_SPIDER_BITE);
    CreatureTemplate {
        name: "Giant Wolf Spider",
        // 'x' (lowercase) — small-arachnid silhouette mirroring the
        // regular Spider's 'X' (uppercase) at one size category
        // smaller. The capital 'X' on the regular spider reads as
        // "medium-large web-spinner"; lowercase 'x' reads as "smaller
        // ground-burrow hunter" at the same UI scale. Distinct from
        // 'X' (Spider) and unrelated to any other lowercase 'x' user.
        glyph: 'x',
        ac: 13,
        // 2d8+2 = 11 average per MM (CR ¼).
        hitpoints: "2d8+2".parse().unwrap(),
        speed: 40.,
        strength: 12,
        intelligence: 3,
        dexterity: 16,
        wisdom: 12,
        constitution: 13,
        charisma: 4,
        skills: HashSet::from([Skill::Perception, Skill::Stealth]),
        senses: HashSet::from([
            SpecialSense::Blindsight(10),
            SpecialSense::Darkvision(60),
        ]),
        cr: 0.25,
        size: Size::Medium,
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
            &GIANT_WOLF_SPIDER_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn giant_wolf_spider_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.25);
        assert_eq!(a.size(), Size::Medium);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("giant wolf spider bite").is_some());
    }

    #[test]
    fn giant_wolf_spider_carries_arachnid_senses() {
        // Pin the load-bearing sensory trait: Blindsight 10 +
        // Darkvision 60 — the standard arachnid sensory envelope
        // shared with the regular Spider. The blindsight in
        // particular matters for the engine's invisibility / illusion
        // concealment gates — a creature with blindsight pierces
        // those concealment lanes at melee range.
        let a = make();
        assert!(a.senses().contains(&SpecialSense::Blindsight(10)));
        assert!(a.senses().contains(&SpecialSense::Darkvision(60)));
    }
}
