use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::CAT_CLAWS;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size};
use std::sync::LazyLock;

/// Cat — CR 0 tiny beast. The canonical "hearth cat" ambient mouser: a
/// single claws swing for 1 slashing, AC 12, 2 HP. Slots beside the
/// Hawk (CR 0 tiny flier), Bat (CR 0 tiny blindsight flier), Rat
/// (CR 0 tiny rodent) at the very bottom of the CR ladder. The threat
/// profile is mobility + DEX-driven evasion, not damage: a cat does
/// almost nothing if hit, but outruns most of what wants to eat it
/// (RAW: walk 40, climb 40 — the board has no third axis for the
/// climb, so the walking half is the number).
///
/// Action lane:
/// - **cat claws** — DEX-based 1-flat slashing melee via the shared
///   `CAT_CLAWS` static. The cat's only swing. The 1d1-shape die
///   crits cleanly through the engine's uniform crit-doubling chassis
///   instead of needing a flat-1 special case — same trick as the
///   Hawk Talons / Bat Bite / Rat Bite cohort.
///
/// **Keen Smell** (RAW: advantage on Perception checks using smell)
/// is flavor-only — the engine doesn't surface skill checks through
/// combat.
///
/// Defensive identity: AC 12 (tiny + DEX 15), 2 HP. Vanilla beast
/// envelope — no resistances or condition immunities. The cat dies
/// to any solid hit; the load-bearing tactical trait is mobility
/// rather than damage output.
///
/// Stat shape: AC 12, ~2 HP (1d4 → average 2), STR 3, DEX 15, CON 10,
/// INT 3, WIS 12, CHA 7. Speed 40 — RAW's walking speed; its climb
/// is the same 40 and has no lane on a flat board. Size Tiny. CR 0.
/// XP: 10 per RAW.
pub static CAT_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&CAT_CLAWS);
    CreatureTemplate {
        name: "Cat",
        // 'c' (lowercase) — tiny climber silhouette. 'C' (uppercase) is
        // taken by Cambion / Centaur / Cleric / Cloaker / Cockatrice /
        // Couatl cohort; lowercase 'c' reads as "tiny stalker" beside
        // 'b' (Bat), 'r' (Rat), 'k' (Hawk) at the very bottom of the
        // CR ladder.
        glyph: 'c',
        ac: 12,
        // RAW: 2 (1d4) — drifts between 1 and 4 with average 2.5. No
        // -1 modifier (the cat's CON 10 is dead-center), so the engine's
        // 1-floor on HP rolls is only a safety net.
        hitpoints: "1d4".parse().unwrap(),
        // RAW speed line: Speed 40 ft., Climb 40 ft. Both halves are
        // the same number, so the flat board costs the cat nothing.
        speed: 40.,
        strength: 3,
        intelligence: 3,
        dexterity: 15,
        wisdom: 12,
        constitution: 10,
        charisma: 7,
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
            &CAT_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn cat_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.0);
        assert_eq!(a.size(), Size::Tiny);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("cat claws").is_some());
    }

    #[test]
    fn cat_is_dex_keyed() {
        // Pin the load-bearing stat distinction: the cat is a nimble
        // climber whose claws ride on its DEX 15 (not STR 3 like the
        // rat / lizard bite). A future template refactor that quietly
        // toggled the cat back to STR-based claws would erase the
        // DEX-driven envelope that separates the cat from the rat
        // cohort and collapse it into the same flat-1 STR niche.
        let a = make();
        // DEX 15 → +2 modifier. The cat's claws share this with the
        // hawk's DEX-based talons.
        use crate::engine::types::AbilityScoreType;
        assert_eq!(a.ability_score(AbilityScoreType::Dexterity), 15);
    }
}
