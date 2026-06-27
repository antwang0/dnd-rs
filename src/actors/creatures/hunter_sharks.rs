use crate::actions::class_features::BLOOD_FRENZY_TAG;
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::HUNTER_SHARK_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Hunter Shark — CR 2 large beast. The "solitary great white"
/// middle entry on the shark ladder: between the Reef Shark (CR ½
/// medium pack-tactics swarmer) and the Giant Shark (CR 5 huge apex
/// predator). The defining trait is **Blood Frenzy** — every melee
/// swing against a wounded target (HP < max HP) rolls at advantage.
/// Combined with the heavier 2d8+STR bite, the hunter shark snowballs
/// hard on whoever it draws blood from first.
///
/// Action lane:
/// - **hunter shark bite** — STR-based 2d8+STR piercing melee via the
///   shared `HUNTER_SHARK_BITE` static. Vanilla SimpleWeapon — no
///   rider; the snowball multiplier lives on the template-level
///   `BLOOD_FRENZY_TAG` passive feature. Single swing per Action
///   (no multiattack); the shark's threat profile is "one big
///   chomp that gets better as the target bleeds."
///
/// **Water Breathing** (RAW: can breathe only underwater) is
/// flavor-only — the engine doesn't model the aquatic-vs-land
/// terrain split, so the clause collapses to the per-creature
/// 40 walking speed (RAW: swim 40, no land speed; we use the
/// swim speed as the per-creature speed since hunter sharks
/// never walk).
///
/// Defensive identity: AC 12 (large + 13 DEX), 45 HP (6d10+12).
/// Vanilla beast envelope — no resistances or condition
/// immunities. **Blindsight 30** lets the shark hunt by water-
/// pressure sensing through the engine's invisibility / illusion
/// concealment chokepoint at short range. The hunter shark soaks
/// 45 HP and outpaces all but the heaviest CR-2 swings, which
/// gives Blood Frenzy time to ramp up on whichever target it
/// scratches first.
///
/// Stat shape: AC 12, ~45 HP (6d10+12), STR 18, DEX 13, CON 15,
/// INT 1, WIS 10, CHA 4. Speed 40 (RAW swim 40; collapsed to
/// per-creature speed). Senses: Blindsight 30. Size Large. CR 2.
/// XP: 450 per RAW.
pub static HUNTER_SHARK_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&HUNTER_SHARK_BITE);
    CreatureTemplate {
        name: "Hunter Shark",
        // 'Q' (uppercase) — middle rung of the shark silhouette
        // ladder: lowercase 'q' (Reef Shark, medium) → uppercase 'Q'
        // (Hunter Shark, large) → uppercase 'Q' again (Giant Shark,
        // huge). The 'Q' is free in the large-beast slot. The lower
        // / uppercase split mirrors how giant-rat ('r') vs adult-red
        // dragon ('R') already differentiates by size in the engine.
        glyph: 'Q',
        ac: 12,
        // 6d10+12 = 45 average per MM (CR 2).
        hitpoints: "6d10+12".parse().unwrap(),
        speed: 40.,
        strength: 18,
        intelligence: 1,
        dexterity: 13,
        wisdom: 10,
        constitution: 15,
        charisma: 4,
        senses: HashSet::from([SpecialSense::Blindsight(30)]),
        cr: 2.0,
        size: Size::Large,
        creature_type: CreatureType::Beast,
        actions,
        // Blood Frenzy: advantage on melee attacks vs wounded
        // targets. Shared with the Sahuagin / Giant Shark cohort
        // through the engine's BLOOD_FRENZY_TAG feature tag — same
        // chokepoint in `compute_attack_mode`, no new branch in the
        // engine for this template.
        features: HashSet::from([BLOOD_FRENZY_TAG]),
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
            &HUNTER_SHARK_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn hunter_shark_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 2.0);
        assert_eq!(a.size(), Size::Large);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("hunter shark bite").is_some());
    }

    #[test]
    fn hunter_shark_has_blood_frenzy() {
        // Pin the load-bearing combat trait: Blood Frenzy is what
        // separates the hunter shark from a vanilla heavy biter — the
        // snowball-on-blood multiplier IS the shark's combat identity.
        // A future template-refactor that quietly stripped the
        // feature would erase the "frenzies on a scratch" flavor and
        // demote the hunter shark to "a slow large biter with no
        // tricks." Routes through the same chokepoint
        // (`BLOOD_FRENZY_TAG` in `compute_attack_mode`) as the
        // Sahuagin cohort.
        let a = make();
        assert!(a.has_passive_feature(BLOOD_FRENZY_TAG));
    }
}
