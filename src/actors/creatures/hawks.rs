use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::HAWK_TALONS;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size};
use std::sync::LazyLock;

/// Hawk — CR 0 tiny beast. The canonical "flying scout" ambient beast:
/// a single talons swing for 1 slashing, AC 13, 1 HP. Fills the
/// tiny-flier slot at the very bottom of the encounter ladder beside
/// the Stirge (CR ⅛ flying blood-drainer) and the Giant Crab (CR ⅛
/// crawling pincer). The threat profile is mobility, not damage: a
/// hawk does almost nothing if hit, but its fly speed of 60 lets it
/// pick targets at will on an open map.
///
/// Action lane:
/// - **hawk talons** — DEX-based 1-flat slashing melee via the shared
///   `HAWK_TALONS` static. RAW: "Melee Weapon Attack: +5 to hit, reach
///   5 ft, one target. Hit: 1 slashing damage." We use a 1d1-shape
///   die so a confirmed crit doubles to 2 — matching the engine's
///   uniform crit-doubling chassis instead of a special-case flat-1
///   path. Single swing per Action (no multiattack); the load-bearing
///   threat is the bird's mobility.
///
/// **Keen Sight** (RAW: advantage on Perception checks using sight)
/// is flavor-only — the engine doesn't surface skill checks through
/// combat, so the trait collapses to the per-creature base.
///
/// Defensive identity: AC 13 (tiny + high DEX), 1 HP. Vanilla beast
/// envelope — no resistances or condition immunities. The hawk dies
/// to any solid hit; the role is "skirmish nuisance + scout" rather
/// than a credible damage threat. Pair with a melee predator (Giant
/// Rat / Hyena swarm) so the hawk softens fly-vulnerable casters
/// from above while the pack closes from below.
///
/// Stat shape: AC 13, ~1 HP (1d4-1 → floored at 1), STR 5, DEX 16,
/// CON 8, INT 2, WIS 14, CHA 6. Speed 10 walking + fly 60 (we
/// collapse to fly 60 as the per-creature speed since the engine
/// doesn't track separate ground vs fly speed lanes — the bird
/// almost never walks). Size Tiny. CR 0. XP: 10 per RAW.
pub static HAWK_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&HAWK_TALONS);
    CreatureTemplate {
        name: "Hawk",
        // 'k' (lowercase) — tiny flier silhouette. Free at the small-UI
        // scale; 'H' is taken by Half-Orc / Harpy / Hippogriff /
        // Hobgoblin / Hydra cohort and uppercase 'K' is the Knight /
        // Kraken cohort. Lowercase 'k' reads as "tiny tracker" beside
        // the lowercase 'r' (Giant Rat), 's' (Stirge), 'f' (Giant
        // Frog) at the bottom of the CR ladder.
        glyph: 'k',
        ac: 13,
        // RAW: 1 (1d4 - 1) — drifts between 1 and 3 with average 1.5.
        // The engine's HP-roll path floors at 1 (a fresh spawn must
        // be alive) so an unlucky d4 = 1 still spawns the hawk with
        // 1 HP rather than dead.
        hitpoints: "1d4-1".parse().unwrap(),
        // Fly 60 — the bird's signature mobility. The engine collapses
        // ground + fly to a single per-creature speed; we pin to the
        // fly speed since hawks almost never walk in encounter scope.
        speed: 60.,
        strength: 5,
        intelligence: 2,
        dexterity: 16,
        wisdom: 14,
        constitution: 8,
        charisma: 6,
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
            &HAWK_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn hawk_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.0);
        assert_eq!(a.size(), Size::Tiny);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("hawk talons").is_some());
    }

    #[test]
    fn hawk_is_fast_flier() {
        // Pin the load-bearing mobility trait: a hawk's combat identity
        // is "free positioning, almost no damage". A future template-
        // refactor that quietly dropped the speed back to the default
        // 30 would erase the bird-of-prey identity (the hawk would
        // become a smaller weaker giant rat). The speed 60 is the load-
        // bearing tactical clause.
        let a = make();
        assert!(a.speed() >= 60.0);
    }
}
