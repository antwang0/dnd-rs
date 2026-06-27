use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{NEEDLE_BLIGHT_CLAWS, NEEDLE_BLIGHT_NEEDLES};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Needle Blight — CR ¼ medium plant. The "thorn switch-hitter" middle
/// tier of the blight ladder: a medium plant with a 1d4 claws melee
/// option AND a 2d6 ranged needle volley, picking its lane by
/// engagement distance. Slots between the Twig Blight (CR ⅛ small,
/// melee-only claws) and the Vine Blight (CR ½ medium, constrict-
/// restrain grapple lock) — the middle entry on the blight family
/// ladder that completes the canonical "evil-druid grove" set.
///
/// Action lanes:
/// - **needle blight claws** — STR-based 2d4+STR piercing melee via
///   the shared `NEEDLE_BLIGHT_CLAWS` static (`SimpleWeapon::melee`).
///   Heavier than the twig's 1d4 — the medium frame's signature in-
///   melee swing.
/// - **needle blight needles** — STR-based 2d6+STR piercing ranged via
///   the shared `NEEDLE_BLIGHT_NEEDLES` static (`SimpleWeapon::ranged`,
///   12-tile normal / 24-tile long range = 30/60ft). The load-bearing
///   ranged option that lets the blight pressure back-line targets
///   where the twig blight can't follow. Heavier dice than the claws
///   to mirror RAW's "ranged volley is the threat" design.
///
/// **False Appearance** (RAW: indistinguishable from a normal shrub
/// while motionless) is flavor-only — the engine doesn't model
/// ambient-environment camouflage outside the Mimic's targeted
/// reveal-on-attack envelope. The Blindsight 60 cone covers the
/// load-bearing sensory niche.
///
/// Defensive identity: AC 12 (medium + no DEX), 11 HP (2d8+2).
/// **Blindsight 60** lets the needle blight "see" via vibration in
/// pitch-black biomes — it's blind RAW (no eyes) so blindsight is the
/// load-bearing sense. **Fire vulnerability** (the canonical
/// burnable-plant weakness shared with Twig / Vine Blight). **Blinded +
/// Deafened condition immunity** (RAW: no eyes / no ears) round out the
/// family's defensive envelope.
///
/// Stat shape: AC 12, ~11 HP (2d8+2), STR 12, DEX 12, CON 13, INT 4,
/// WIS 8, CHA 5. Speed 30 (faster than twig at 20 / vine at 10 —
/// the middle-tier blight is the most mobile of the three). Languages:
/// Common (RAW: "understands Common but can't speak"). Senses:
/// Blindsight 60. Size Medium. CR ¼. XP: 50 per RAW.
pub static NEEDLE_BLIGHT_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&NEEDLE_BLIGHT_CLAWS);
    actions.push(&NEEDLE_BLIGHT_NEEDLES);
    let mut damage_modifiers: HashMap<DamageType, DamageModifier> = HashMap::new();
    // Canonical burnable-plant weakness shared with the rest of the
    // blight family: fire-typed hits land at 2x damage.
    damage_modifiers.insert(DamageType::Fire, DamageModifier::Vulnerability);
    let condition_immunities = HashSet::from([
        // RAW: no eyes — Blinded is meaningless.
        Condition::Blinded,
        // RAW: no ears — Deafened is meaningless.
        Condition::Deafened,
    ]);
    CreatureTemplate {
        name: "Needle Blight",
        // 'N' (uppercase) — distinct from 'n' (taken by Nothic / other
        // small-frame entries) and from 'V' (Vine Blight, medium) /
        // 't' (Twig Blight, small). Uppercase 'N' reads as "medium
        // bristly silhouette" at the small UI scale alongside Nightmare
        // / Nalfeshnee on the 'N' bench.
        glyph: 'N',
        ac: 12,
        // 2d8+2 = 11 average per MM (CR ¼).
        hitpoints: "2d8+2".parse().unwrap(),
        // Speed 30 — the fastest of the three blights. The medium-tier
        // entry trades the vine blight's grapple lockdown for actual
        // mobility on the battlefield.
        speed: 30.,
        strength: 12,
        intelligence: 4,
        dexterity: 12,
        wisdom: 8,
        constitution: 13,
        charisma: 5,
        senses: HashSet::from([SpecialSense::Blindsight(60)]),
        languages: HashSet::from([Language::Common]),
        cr: 0.25,
        size: Size::Medium,
        creature_type: CreatureType::Plant,
        actions,
        damage_modifiers,
        condition_immunities,
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
            &NEEDLE_BLIGHT_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn needle_blight_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.25);
        assert_eq!(a.size(), Size::Medium);
        assert_eq!(a.creature_type(), CreatureType::Plant);
        assert!(a.find_action("needle blight claws").is_some());
        assert!(a.find_action("needle blight needles").is_some());
    }

    #[test]
    fn needle_blight_carries_both_melee_and_ranged() {
        // Pin the load-bearing switch-hitter identity: the needle blight
        // has BOTH a melee claws lane AND a ranged needle volley lane.
        // A future template refactor that quietly dropped either would
        // erase the "engagement-distance picks your lane" niche that
        // distinguishes the needle blight from the melee-only twig and
        // the grapple-only vine blight.
        let a = make();
        assert!(a.find_action("needle blight claws").is_some());
        assert!(a.find_action("needle blight needles").is_some());
    }

    #[test]
    fn needle_blight_is_fire_vulnerable() {
        // Pin the family-wide burnable-plant weakness.
        let a = make();
        assert_eq!(
            a.damage_modifier(DamageType::Fire),
            Some(DamageModifier::Vulnerability)
        );
    }

    #[test]
    fn needle_blight_carries_blindsight() {
        // Pin the load-bearing sensory trait: Blindsight 60 covers
        // the blight's full effective sense radius (it's blind RAW).
        let a = make();
        assert!(a.senses().contains(&SpecialSense::Blindsight(60)));
    }

    #[test]
    fn needle_blight_is_immune_to_blinded_and_deafened() {
        // Pin the no-eyes / no-ears condition immunities — shared
        // across the entire blight family.
        let a = make();
        assert!(a.is_immune_to_condition(Condition::Blinded));
        assert!(a.is_immune_to_condition(Condition::Deafened));
    }
}
