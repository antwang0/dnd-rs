use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{GALEB_DUHR_MULTI, GALEB_DUHR_SLAM};
use crate::actors::actor_template::CreatureTemplate;
use crate::actors::creatures::fire_elementals::{
    elemental_defaults,
};
use crate::engine::types::{CreatureType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Galeb Duhr — CR 6 medium elemental. The granite-guardian of the
/// Plane of Earth — a bipedal boulder with a permanent scowl carved into
/// its rough-hewn face. Slots between Otyugh (CR 5) and Drider (CR 6)
/// on the mid-tier ladder — the stone-cousin of the Earth Elemental
/// (CR 5) compressed to a medium frame but with Magic Resistance on
/// top, so the per-encounter threat profile is "Earth Elemental's
/// slams + caster-disruption envelope" at the cost of one less HP die.
///
/// Action lanes:
/// - **galeb duhr multiattack** — 2 slam swings per Action via
///   `Multiattack`. Same single-sub shape as Stone Golem / Iron Golem /
///   Helmed Horror multis; each swing rolls its own d20 + STR vs AC for
///   ~2 × (3d8 + STR) = ~38 average per Action against a single target.
/// - **galeb duhr slam** (standalone) — STR-based 3d8+STR bludgeoning
///   melee, reach 1. Heaviest single-die-tier melee in the mid-CR pool:
///   the 3d8 base + STR-20 modifier produces a brute-force per-swing
///   profile that matches the galeb duhr's "rock with arms" silhouette.
///
/// Defensive identity: AC 16 (natural armor — granite skin), 85 HP
/// (9d8+45). Non-magical BPS resistance + Poison immunity via the
/// shared `elemental_defaults` baseline; Magic Resistance is
/// the variant-specific overlay (advantage on saves vs spells / magical
/// effects, the canonical caster-disruption lane). Full elemental
/// condition envelope (Charmed / Frightened / Paralyzed / Petrified /
/// Poisoned / Asleep / Prone / Grappled / Restrained) shared via
/// `ELEMENTAL_CONDITION_IMMUNITIES`.
///
/// Stat shape: AC 16, ~85 HP (9d8+45), STR 20, DEX 6, CON 20, INT 11,
/// WIS 12, CHA 11. Speed 15 (the granite mass moves slow — half the
/// Earth Elemental's 30-ft speed since the duhr is the "static boulder"
/// flavor). Senses: Darkvision 60, Tremorsense 60 (the duhr feels
/// through stone). Languages: Terran (collapsed to Primordial in the
/// engine's `Language` enum). Size Medium. CR 6.
///
/// RAW also gives the galeb duhr:
/// - **Animate Boulders** (recharge 6: animate two boulders within
///   60 ft as a kind of "stone golem of stone golems") — omitted since
///   the engine doesn't model summoning new actors mid-combat through
///   an Action (the Animate Objects spell exists but its scaffolding
///   is single-target only).
/// - **Rolling Charge** (move 20+ ft straight then slam for +3d6 extra
///   bludgeoning + DC 13 STR or knocked Prone) — omitted because the
///   engine doesn't track per-Action movement-vector history that a
///   "straight line ≥ 20 ft" predicate would need.
/// - **False Appearance** (indistinguishable from a normal boulder while
///   motionless) — no in-engine consumer; the AI doesn't model
///   visual-disguise checks against approaching enemies.
pub static GALEB_DUHR_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*GALEB_DUHR_MULTI);
    actions.push(&GALEB_DUHR_SLAM);
    CreatureTemplate {
        name: "Galeb Duhr",
        // 'q' (lowercase) — distinct from 'Q' (Earth Elemental uses
        // uppercase). 'q' for the smaller-but-magic-resistant cousin
        // reads as the bipedal boulder silhouette.
        glyph: 'q',
        ac: 16,
        // 9d8+45 ≈ 85 average per MM (CR 6).
        hitpoints: "9d8+45".parse().unwrap(),
        // 15 ft RAW — half the Earth Elemental's speed; the duhr is
        // the "static boulder" flavor.
        speed: 15.,
        strength: 20,
        intelligence: 11,
        dexterity: 6,
        wisdom: 12,
        constitution: 20,
        charisma: 11,
        senses: HashSet::from([
            SpecialSense::Darkvision(60),
            SpecialSense::Tremorsense(60),
        ]),
        languages: HashSet::from([Language::Primordial]),
        cr: 6.0,
        size: Size::Medium,
        creature_type: CreatureType::Elemental,
        actions,
        // Shared elemental damage envelope (poison immunity + non-magical
        // BPS resistance); no signature overlay — the duhr is a stone
        // elemental that DOESN'T have an elemental damage type-affinity
        // (unlike the fire / water / earth-elemental's thunder rider).
        // 5e Magic Resistance — advantage on saves vs spells / magical
        // effects. The duhr's variant-specific defensive lane that
        // distinguishes it from the vanilla Earth Elemental.
        has_magic_resistance: true,
        ..elemental_defaults([])
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::conditions::Condition;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::{Coordinate, DamageModifier, DamageType};

    #[test]
    fn galeb_duhr_template_shape() {
        let a = ActorInstance::from_creature_template(
            &GALEB_DUHR_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 6.0);
        assert_eq!(a.size(), Size::Medium);
        assert_eq!(a.creature_type(), CreatureType::Elemental);
        // The duhr's two action lanes — multi (double slam) primary,
        // single slam for AI fallback.
        assert!(a.find_action("galeb duhr multiattack").is_some());
        assert!(a.find_action("galeb duhr slam").is_some());
    }

    #[test]
    fn galeb_duhr_has_magic_resistance_and_elemental_envelope() {
        let a = ActorInstance::from_creature_template(
            &GALEB_DUHR_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        // Magic Resistance — the signature variant-specific defensive
        // lane (advantage on saves vs spells / magical effects).
        assert!(a.has_magic_resistance());
        // Standard elemental envelope: poison immunity + non-magical
        // BPS resistance.
        assert_eq!(
            a.damage_modifier(DamageType::Poison),
            Some(DamageModifier::Immunity)
        );
        assert_eq!(
            a.nonmagical_damage_modifier(DamageType::Bludgeoning),
            Some(DamageModifier::Resistance)
        );
        // Condition envelope inherited from the elemental constant.
        assert!(a.effectively_immune_to_condition(Condition::Charmed));
        assert!(a.effectively_immune_to_condition(Condition::Paralyzed));
        assert!(a.effectively_immune_to_condition(Condition::Petrified));
    }
}
