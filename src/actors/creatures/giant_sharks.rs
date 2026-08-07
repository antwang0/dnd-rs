use crate::actions::class_features::{BLOOD_FRENZY_TAG, SWIM_SPEED_TAG};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::GIANT_SHARK_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Giant Shark — CR 5 huge beast. The apex of the shark ladder:
/// above the Reef Shark (CR ½ medium pack-swarmer) and the Hunter
/// Shark (CR 2 large solo-frenzy biter). The defining trait is
/// still **Blood Frenzy** — every melee swing against a wounded
/// target rolls at advantage — but the giant shark layers it on
/// top of a much heavier 3d10+STR bite that already deletes most
/// medium-AC targets in a single chomp before the frenzy lane
/// even activates.
///
/// Action lane:
/// - **giant shark bite** — STR-based 3d10+STR piercing melee via
///   the shared `GIANT_SHARK_BITE` static. Vanilla SimpleWeapon —
///   no rider; the snowball multiplier still rides on the
///   template-level `BLOOD_FRENZY_TAG` passive. Single swing per
///   Action (no multiattack); a giant shark only needs one bite.
///   Average per hit: 3d10+5 = ~21.5 (advantage rounds drift
///   upward by ~25%), enough to one-shot most low-CR PCs and
///   leave a clean half-HP heavyweight for the frenzy to chew on
///   next round.
///
/// **Water Breathing** (RAW: can breathe only underwater) is
/// flavor-only — the engine doesn't model the aquatic-vs-land
/// terrain split, so the clause collapses to the per-creature
/// 50 walking speed (RAW: swim 50, no land speed; we use the
/// swim number as the per-creature speed since giant sharks
/// never walk).
///
/// Defensive identity: AC 13 (huge + 11 DEX), 126 HP (11d12+44).
/// Vanilla beast envelope — no resistances or condition
/// immunities. **Blindsight 60** (twice the reef / hunter shark
/// range) routes the giant shark through the engine's
/// invisibility / illusion concealment chokepoint at a far longer
/// range — a hidden caster can't sneak past a giant shark the
/// way they might dodge a reef shark's pack. The 126-HP frame
/// outlasts most CR-5 swing exchanges, giving Blood Frenzy
/// multiple full rounds to ramp.
///
/// Stat shape: AC 13, ~126 HP (11d12+44), STR 23, DEX 11, CON 19,
/// INT 1, WIS 10, CHA 5. Speed 50 (RAW swim 50; magnitude collapsed to
/// per-creature speed). Senses: Blindsight 60. Size Huge. CR 5.
/// XP: 1800 per RAW.
pub static GIANT_SHARK_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GIANT_SHARK_BITE);
    CreatureTemplate {
        name: "Giant Shark",
        // 'Q' (uppercase) — top of the shark silhouette ladder.
        // Shared with the Hunter Shark on purpose: the size + team
        // color disambiguate on the map, and a giant shark next to a
        // hunter shark next to a reef shark reads as "three shark
        // sizes" at a glance via the same glyph family. 'S' is taken
        // (Sahuagin / Stone Giant / Storm Giant); 'G' is taken
        // (Giant cohort generally). The reef / hunter / giant 'q' /
        // 'Q' / 'Q' family parallels the rat 'r' / 'R' (Adult Red)
        // size-vs-type split.
        glyph: 'Q',
        ac: 13,
        // 11d12+44 = 126 average per MM (CR 5).
        hitpoints: "11d12+44".parse().unwrap(),
        speed: 50.,
        strength: 23,
        intelligence: 1,
        dexterity: 11,
        wisdom: 10,
        constitution: 19,
        charisma: 5,
        senses: HashSet::from([SpecialSense::Blindsight(60)]),
        cr: 5.0,
        size: Size::Huge,
        creature_type: CreatureType::Beast,
        actions,
        // Blood Frenzy — same `BLOOD_FRENZY_TAG` chokepoint as the
        // Hunter Shark / Sahuagin. The trait is what separates the
        // giant shark from a vanilla huge biter at CR 5; combined
        // with the 3d10 base die it makes the shark's second swing
        // against a wounded target the deadliest non-recharge melee
        // attack at the CR-5 tier.
        features: HashSet::from([SWIM_SPEED_TAG, BLOOD_FRENZY_TAG]),
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
            &GIANT_SHARK_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn giant_shark_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 5.0);
        assert_eq!(a.size(), Size::Huge);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("giant shark bite").is_some());
    }

    #[test]
    fn giant_shark_has_blood_frenzy_and_long_blindsight() {
        // Pin the load-bearing combat + sensory traits: Blood Frenzy
        // is the snowball-on-blood multiplier shared with the Hunter
        // Shark cohort, and Blindsight 60 (twice the smaller sharks'
        // range) is the load-bearing "no invisibility cheese" gate
        // at the CR-5 tier. A future template-refactor that quietly
        // stripped either would silently demote the giant shark.
        let a = make();
        assert!(a.has_passive_feature(BLOOD_FRENZY_TAG));
        assert!(a.senses().contains(&SpecialSense::Blindsight(60)));
    }
}
