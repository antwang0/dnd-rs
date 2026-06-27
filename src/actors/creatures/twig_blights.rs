use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::TWIG_BLIGHT_CLAWS;
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Twig Blight — CR ⅛ small plant. The "sapling thorn" entry on the
/// blight ladder: a child-sized walking shrub whose threat is its
/// blindsight + fire-vulnerability gimmick, not raw damage. Slots
/// beside the Needle Blight (CR ¼ medium, double-claw + ranged needle
/// volley) and the Vine Blight (CR ½ medium, constrict-restrain
/// grapple lock) as the entry tier of the blight family — the trio
/// covers the canonical "evil-druid grove" encounter shape.
///
/// Action lane:
/// - **twig blight claws** — STR-based 1d4+STR piercing melee via the
///   shared `TWIG_BLIGHT_CLAWS` static (`SimpleWeapon::melee` chassis).
///   The lightest swing in the CR-⅛ pool — half the damage of a
///   Mastiff bite at the same CR. The twig isn't a damage dealer;
///   it's an ambusher whose payoff comes from numbers + the player's
///   short-rest squeeze on healing.
///
/// **False Appearance** (RAW: indistinguishable from a normal sapling
/// while motionless) is flavor-only — the engine doesn't model
/// ambient-environment camouflage outside the Mimic's targeted
/// reveal-on-attack envelope. The Blindsight 60 cone covers the
/// load-bearing sensory niche.
///
/// Defensive identity: AC 13 (small + DEX-driven), 4 HP (1d6+1).
/// **Blindsight 60** lets the twig blight "see" via vibration under
/// canopy / in pitch-black caves — it's blind RAW (no eyes) so
/// blindsight is the load-bearing sense. **Fire vulnerability** (the
/// canonical burnable-plant weakness shared with Vine Blight / Needle
/// Blight): every fire-typed hit lands at double damage via the
/// standard `damage_modifiers` chokepoint. **Blinded condition
/// immunity** (RAW: the blight has no eyes to blind) and **Deafened
/// condition immunity** (no ears to deafen). Together these capture
/// the twig-blight's "thorn-with-blindsight" silhouette.
///
/// Stat shape: AC 13, ~4 HP (1d6+1), STR 6, DEX 13, CON 12, INT 4,
/// WIS 8, CHA 3. Speed 20 (the brittle sapling shuffles slowly — slower
/// than the needle blight's 30 since the twig is smaller and weaker).
/// Languages: Common (RAW: "understands Common but can't speak").
/// Senses: Blindsight 60. Size Small. CR ⅛. XP: 25 per RAW.
pub static TWIG_BLIGHT_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&TWIG_BLIGHT_CLAWS);
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
        name: "Twig Blight",
        // 't' (lowercase) — small frame, distinct from 'V' (Vine Blight,
        // medium) and 'N' (Needle Blight, medium). Lowercase 't' reads
        // as "small skittering shape" at the small UI scale alongside
        // the rest of the CR-⅛ vermin bench.
        glyph: 't',
        ac: 13,
        // 1d6+1 = 4 average per MM (CR ⅛). Lowest HP tier on the
        // blight ladder — fragile glass-cannon stinger profile.
        hitpoints: "1d6+1".parse().unwrap(),
        // Speed 20 — slower than the needle blight (30) and on par with
        // the vine blight (10–20 range). The fragile sapling shuffles.
        speed: 20.,
        strength: 6,
        intelligence: 4,
        dexterity: 13,
        wisdom: 8,
        constitution: 12,
        charisma: 3,
        senses: HashSet::from([SpecialSense::Blindsight(60)]),
        languages: HashSet::from([Language::Common]),
        cr: 0.125,
        size: Size::Small,
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
            &TWIG_BLIGHT_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn twig_blight_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.125);
        assert_eq!(a.size(), Size::Small);
        assert_eq!(a.creature_type(), CreatureType::Plant);
        assert!(a.find_action("twig blight claws").is_some());
    }

    #[test]
    fn twig_blight_is_fire_vulnerable() {
        // Pin the canonical burnable-plant weakness — the blight
        // family's defining "burn the grove" identity. A future
        // refactor that quietly stripped fire vulnerability would
        // erase the blight's signature counter.
        let a = make();
        assert_eq!(
            a.damage_modifier(DamageType::Fire),
            Some(DamageModifier::Vulnerability)
        );
    }

    #[test]
    fn twig_blight_carries_blindsight() {
        // Pin the load-bearing sensory trait: Blindsight 60 covers
        // the blight's full effective sense radius (it's blind RAW).
        let a = make();
        assert!(a.senses().contains(&SpecialSense::Blindsight(60)));
    }

    #[test]
    fn twig_blight_is_immune_to_blinded_and_deafened() {
        // Pin the no-eyes / no-ears condition immunities — shared
        // across the entire blight family.
        let a = make();
        assert!(a.is_immune_to_condition(Condition::Blinded));
        assert!(a.is_immune_to_condition(Condition::Deafened));
    }
}
