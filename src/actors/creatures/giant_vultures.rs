use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::GIANT_VULTURE_MULTI;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Giant Vulture — CR 1 large beast. The "carrion-bird scavenger" tier:
/// a Pack-Tactics flying predator with a heterogeneous beak + talons
/// compound multi for ~11 average damage per Action against a single
/// target. Slots beside the Giant Eagle (CR 1 large beast, single-swing
/// solo flier) on the CR-1 flying-beast bench — the vulture's edge is
/// Pack Tactics, not raw dice.
///
/// Action lane:
/// - **giant vulture multiattack** — STR-based heterogeneous compound
///   per Action: 1 beak (1d4+STR piercing) + 1 talons (2d4+STR slashing)
///   via the shared `GIANT_VULTURE_MULTI` `CompoundAttack`. Same one-
///   Action two-swing shape as the Owlbear / Werewolf / Griffon compounds
///   — the dice differ; the chassis is shared.
///
/// **Keen Sight & Smell** (RAW: advantage on Perception checks using
/// sight or smell) is flavor-only — the engine doesn't surface skill
/// checks through combat. **Flyby** (RAW: doesn't provoke opportunity
/// attacks when it flies out of an enemy's reach) is on, via
/// `FLYBY_TAG` — and the "walking opportunity-attack tracking" the
/// scope cut pointed at is exactly where it lives, as a mover-side
/// suppression beside Disengage. A vulture on the ground still
/// provokes, because RAW's clause says "when it flies".
///
/// Defensive identity: AC 10 (large + no DEX bonus, no natural armor),
/// 25 HP (3d10+9). Vanilla beast envelope — no resistances or condition
/// immunities. The vulture dies to two solid hits; its threat lives in
/// the Pack Tactics multiplier on the compound multi when two or more
/// vultures swarm a single target.
///
/// Stat shape: AC 10, ~25 HP (3d10+9), STR 15, DEX 10, CON 16, INT 6,
/// WIS 12, CHA 7. Speed 10 walking + fly 60 (collapsed to fly 60 as
/// the per-creature speed since the engine doesn't track separate
/// ground / fly speed lanes — the bird almost never walks). Size Large.
/// CR 1. XP: 200 per RAW.
pub static GIANT_VULTURE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*GIANT_VULTURE_MULTI);
    CreatureTemplate {
        name: "Giant Vulture",
        // 'V' (uppercase) — shared with Vrock / Vampire / Veteran cohort;
        // the beast vs fiend/humanoid context disambiguates in the prompt
        // and the team color separates them on the map. Lowercase 'v' is
        // free but harder to read at small UI scale; uppercase 'V' matches
        // the "large flier silhouette" beside Giant Eagle ('E') and the
        // rest of the large-beast bench.
        glyph: 'V',
        ac: 10,
        // 3d10+9 = 25 average per MM (CR 1). Floored at 1 for unlucky
        // dice rolls.
        hitpoints: "3d10+9".parse().unwrap(),
        // Fly 60 — the bird's signature mobility, mirroring Hawk / Giant
        // Eagle. The engine collapses ground + fly to a single per-creature
        // speed; we pin to the fly speed since vultures almost never walk
        // in encounter scope.
        // RAW speed line: Speed 10 ft., fly 60 ft.
        speed: 10.0,
        fly_speed: 60.0,
        strength: 15,
        intelligence: 6,
        dexterity: 10,
        wisdom: 12,
        constitution: 16,
        charisma: 7,
        cr: 1.0,
        size: Size::Large,
        creature_type: CreatureType::Beast,
        actions,
        // 5e Pack Tactics — advantage on the attack roll when an ally
        // is within 5ft of the target. The load-bearing tactical trait
        // that distinguishes the vulture from the solo Giant Eagle:
        // two vultures swarming a wounded target each swing the
        // compound multi at advantage, doubling effective accuracy and
        // crit chance. Routes through the same chokepoint as the Wolf /
        // Kobold / Giant Rat pack lanes.
        has_pack_tactics: true,
        // 5e **Flyby**: "doesn't provoke an opportunity attack when it
        // flies out of an enemy's reach." Read by the mover-side
        // suppression lane in `dispatch_opportunity_attacks`, and gated
        // there on the creature actually being airborne.
        features: HashSet::from([crate::actions::class_features::FLYBY_TAG]),
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
            &GIANT_VULTURE_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn giant_vulture_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 1.0);
        assert_eq!(a.size(), Size::Large);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("giant vulture multiattack").is_some());
    }

    #[test]
    fn giant_vulture_carries_pack_tactics() {
        // Pin the load-bearing tactical trait: Pack Tactics is what
        // distinguishes the vulture from the solo Giant Eagle at the
        // same CR. A future refactor that quietly stripped the flag
        // would silently demote the vulture to "a weaker Giant Eagle".
        let a = make();
        assert!(a.has_pack_tactics());
    }

    #[test]
    fn giant_vulture_is_fast_flier() {
        // Pin the mobility trait: fly 60 is the carrion bird's identity.
        // Dropping it to the default 30 would erase the predator-bird
        // silhouette.
        let a = make();
        assert!(a.speed() >= 60.0);
    }
}
