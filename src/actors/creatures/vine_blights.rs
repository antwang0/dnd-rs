use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::VINE_BLIGHT_CONSTRICT;
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Vine Blight — CR ½ medium plant. The "ambulatory thorn-tangle" tier
/// of the plant lane: a sentient mass of strangling vines whose threat
/// is the constrict-grapple lock-down envelope, not raw damage. Slots
/// beside the Awakened Tree (CR 2 huge plant, double-slam Multi) and
/// the Shambling Mound (CR 5 large plant, restrain-and-absorb) on the
/// plant-monstrosity ladder — the vine blight is the *low-CR entry*
/// covering the same "vegetative ambusher with grapple identity" niche
/// at a smaller frame.
///
/// Action lane:
/// - **vine blight constrict** — STR-based 2d6+STR bludgeoning melee
///   with a DC 12 STR save-or-Restrained rider via the shared
///   `WeaponWithSaveCondition` chassis. We model the RAW "grappled AND
///   restrained" envelope as the single `Restrained` install — the
///   stronger of the two (Restrained already zeros movement, gives
///   attackers advantage, and imposes DEX-save disadvantage), fully
///   covering the Grappled clause. Routes through the same chokepoint
///   as the Wolf-trip / Dire-Wolf-trip / Worg-trip bites; only the
///   condition (Restrained instead of Prone) and the dice differ.
///
/// **Entangling Plants** (RAW: Recharge 5-6, 15ft burst, DC 12 STR
/// save-or-Restrained for all targets in radius) is omitted as a
/// deliberate scope cut — the Constrict bite already pins one target
/// per round via the same Restrained envelope, and adding a burst
/// version at the same CR ½ would inflate the per-round threat above
/// its tier. The Awakened Tree / Treant / Shambling Mound cover the
/// area-restraint plant niche at higher CRs.
///
/// **False Appearance** (RAW: indistinguishable from a normal vine
/// while motionless) is flavor-only — the engine doesn't model
/// ambient-environment camouflage outside the Mimic's targeted
/// reveal-on-attack envelope.
///
/// Defensive identity: AC 12 (medium + no DEX), 26 HP (4d8+8).
/// **Blindsight 60** lets the vine blight "see" via vibration in pitch-
/// black biomes — it's blind RAW (no eyes) so blindsight is the
/// load-bearing sense. **Fire vulnerability** (the canonical
/// burnable-plant weakness): every fire-typed hit lands at double
/// damage via the standard `damage_modifiers` chokepoint. **Lightning
/// resistance** (RAW: lightning passes through the woody plant body
/// without grounding). **Blinded condition immunity** (RAW: the
/// blight has no eyes to blind) and **Deafened condition immunity**
/// (no ears to deafen). Together these capture the vine-blight's
/// "plant-with-blindsight" silhouette.
///
/// Stat shape: AC 12, ~26 HP (4d8+8), STR 15, DEX 8, CON 14, INT 5,
/// WIS 10, CHA 3. Speed 10 (the lurching vine-tangle moves slowly).
/// Languages: Common (RAW: "understands Common but can't speak").
/// Senses: Blindsight 60. Size Medium. CR ½. XP: 100 per RAW.
pub static VINE_BLIGHT_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&VINE_BLIGHT_CONSTRICT);
    let mut damage_modifiers: HashMap<DamageType, DamageModifier> = HashMap::new();
    // Canonical burnable-plant weakness: fire hits at 2x damage.
    damage_modifiers.insert(DamageType::Fire, DamageModifier::Vulnerability);
    // RAW: lightning resistance (the woody plant body doesn't ground
    // the charge cleanly).
    damage_modifiers.insert(DamageType::Lightning, DamageModifier::Resistance);
    let condition_immunities = HashSet::from([
        // RAW: no eyes — Blinded is meaningless.
        Condition::Blinded,
        // RAW: no ears — Deafened is meaningless.
        Condition::Deafened,
    ]);
    CreatureTemplate {
        name: "Vine Blight",
        // 'V' (uppercase) — shared with Giant Vulture / Vrock / Vampire /
        // Veteran cohort; the plant vs beast/fiend/humanoid context
        // disambiguates in the prompt and the team color separates them
        // on the map. The lurching vine-tangle silhouette reads as "tall
        // creeping shape" at the small UI scale.
        glyph: 'V',
        ac: 12,
        // 4d8+8 = 26 average per MM (CR ½).
        hitpoints: "4d8+8".parse().unwrap(),
        // Speed 10 — the vine-tangle creeps. The slow-mover identity is
        // load-bearing: a fleeing PC can outpace a vine blight unless
        // they're already grappled by the constrict rider, which is
        // exactly the trade the creature's tactical kit is built around.
        speed: 10.,
        strength: 15,
        intelligence: 5,
        dexterity: 8,
        wisdom: 10,
        constitution: 14,
        charisma: 3,
        // Blindsight 60 — the load-bearing sensory trait. The blight
        // is blind RAW; blindsight covers its full effective sense
        // radius. Routes through the same concealment-cancelling
        // chokepoint as the rest of the blindsight pool.
        senses: HashSet::from([SpecialSense::Blindsight(60)]),
        languages: HashSet::from([Language::Common]),
        cr: 0.5,
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
            &VINE_BLIGHT_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn vine_blight_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.5);
        assert_eq!(a.size(), Size::Medium);
        assert_eq!(a.creature_type(), CreatureType::Plant);
        assert!(a.find_action("vine blight constrict").is_some());
    }

    #[test]
    fn vine_blight_is_fire_vulnerable() {
        // Pin the canonical burnable-plant weakness: fire-typed hits
        // land at 2x damage. A future template refactor that quietly
        // stripped fire vulnerability would silently erase the plant's
        // defining "burn the woods to flush them" identity.
        let a = make();
        assert_eq!(
            a.damage_modifier(DamageType::Fire),
            Some(DamageModifier::Vulnerability)
        );
    }

    #[test]
    fn vine_blight_resists_lightning() {
        // Pin the woody-plant lightning resistance — the canonical
        // companion to fire vulnerability on the plant lane.
        let a = make();
        assert_eq!(
            a.damage_modifier(DamageType::Lightning),
            Some(DamageModifier::Resistance)
        );
    }

    #[test]
    fn vine_blight_carries_blindsight() {
        // Pin the load-bearing sensory trait: Blindsight 60 covers
        // the blight's full effective sense radius (it's blind RAW).
        let a = make();
        assert!(a.senses().contains(&SpecialSense::Blindsight(60)));
    }

    #[test]
    fn vine_blight_is_immune_to_blinded_and_deafened() {
        // Pin the no-eyes / no-ears condition immunities: a future
        // refactor stripping them would let a Blindness-on-hit ability
        // erroneously land on a creature that has no eyes RAW.
        let a = make();
        assert!(a.is_immune_to_condition(Condition::Blinded));
        assert!(a.is_immune_to_condition(Condition::Deafened));
    }
}
