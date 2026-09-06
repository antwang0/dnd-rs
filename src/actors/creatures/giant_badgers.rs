use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{GIANT_BADGER_BITE, GIANT_BADGER_CLAWS, GIANT_BADGER_MULTI};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Giant Badger — CR ¼ medium beast. The "burrowing tunneler" tier of
/// stout woodland mustelid: a chunky-jawed digger with a heterogeneous
/// bite + claws compound multi for ~10 average damage per Action. Slots
/// beside the Giant Frog (CR ¼ swamp ambusher), Giant Lizard (CR ¼
/// dungeon-mount), Giant Wolf Spider (CR ¼ venom-rider crawler), and
/// Giant Centipede (CR ¼ small venom-glass-cannon) on the CR-¼ beast
/// bench. Distinct from the Giant Rat (CR ⅛ Pack-Tactics swarmer)
/// because it's a *single tough mustelid*, not a swarm — the threat
/// profile is "one chunky stat block, two limb types per swing,"
/// punctuating its niche as a "fortress in the burrow" rather than a
/// pack predator.
///
/// Action lanes:
/// - **giant badger multiattack** — heterogeneous compound per Action:
///   1 bite (1d6+STR piercing) + 1 claws (2d4+STR slashing) via the
///   shared `GIANT_BADGER_MULTI` `CompoundAttack`. Same one-Action two-
///   swing shape as the Owlbear / Werewolf / Griffon / Giant Vulture
///   compounds — the dice differ; the chassis is shared. The dual-type
///   damage spread (piercing + slashing) means a piercing-resistant
///   target still eats the claws at full value, and vice versa.
/// - **giant badger bite** (standalone) — single-swing fallback for AI
///   scripts that step into adjacency between the multi swings. STR-
///   based 1d6+STR piercing melee via `GIANT_BADGER_BITE`.
/// - **giant badger claws** (standalone) — heavier single-swing
///   fallback. STR-based 2d4+STR slashing melee via `GIANT_BADGER_CLAWS`.
///
/// **Keen Smell** (RAW: advantage on Perception checks using smell)
/// is flavor-only — the engine doesn't surface skill checks through
/// combat.
///
/// Defensive identity: AC 13 (medium + no DEX bonus, hide), 15 HP
/// (2d8+6). Vanilla beast envelope — no resistances or condition
/// immunities. The badger takes two solid hits to drop; threat lives in
/// the heterogeneous compound multi against single targets and the
/// dual-typed damage spread (bypasses most single-type resistances).
///
/// Stat shape: AC 13, ~15 HP (2d8+6), STR 15, DEX 10, CON 17, INT 2,
/// WIS 12, CHA 5. Speed 30 — RAW: 30 walking + burrow 10. The engine
/// doesn't track separate burrow speed (no underground-terrain
/// awareness), so the burrow half is collapsed to the walking value.
/// Senses: Darkvision 30. Size Medium. CR ¼. XP: 50 per RAW.
pub static GIANT_BADGER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*GIANT_BADGER_MULTI);
    actions.push(&GIANT_BADGER_BITE);
    actions.push(&GIANT_BADGER_CLAWS);
    CreatureTemplate {
        name: "Giant Badger",
        // 'B' (uppercase) — shared with Giant Bat / Brown Bear / Bugbear /
        // Bandit Captain cohort. The team color disambiguates on the
        // map and the beast / medium-CR-¼ context separates it from the
        // same-glyph humanoid / fiend cohort. Uppercase 'B' reads as
        // "stout four-legged digger silhouette" at the small UI scale.
        glyph: 'B',
        ac: 13,
        // 2d8+6 = 15 average per MM (CR ¼).
        hitpoints: "2d8+6".parse().unwrap(),
        speed: 30.,
        strength: 13,
        intelligence: 2,
        dexterity: 10,
        wisdom: 12,
        constitution: 17,
        charisma: 5,
        // Darkvision 30 — the burrow-dweller's low-light senses
        // (matches the Giant Frog's 30-ft envelope at the same CR
        // tier). Routes through the standard `SpecialSense::Darkvision`
        // chokepoint.
        senses: HashSet::from([SpecialSense::Darkvision(30)]),
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
            &GIANT_BADGER_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn giant_badger_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.25);
        assert_eq!(a.size(), Size::Medium);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        // Three action lanes — multi (bite + claws) primary, bite +
        // claws standalone for the AI fallback.
        assert!(a.find_action("giant badger multiattack").is_some());
        assert!(a.find_action("giant badger bite").is_some());
        assert!(a.find_action("giant badger claws").is_some());
    }
}
