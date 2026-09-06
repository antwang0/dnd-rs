use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{FLESH_GOLEM_MULTI, FLESH_GOLEM_SLAM};
use crate::actors::actor_template::{CreatureTemplate, DamageFlinch, damage_modifiers_from};
use crate::conditions::{Condition, ConditionTimer};
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Flesh Golem — CR 5 medium construct. The patchwork servitor of mad
/// alchemists and necromancers: stitched-together corpses animated by
/// elemental lightning. Slots between Salamander (CR 5) and Galeb Duhr
/// (CR 6) on the mid-tier monster ladder, and below Stone Golem (CR 10) /
/// Iron Golem (CR 16) on the construct ladder — the cheapest entry on
/// the canonical four-golem family (flesh / clay / stone / iron).
///
/// Action lanes:
/// - **flesh golem multiattack** — 2 slams per Action via `Multiattack`.
///   Roughly 25 average damage per Action against a single target — the
///   golem's main job is to crush whoever stays in melee range.
/// - **flesh golem slam** (standalone) — STR-based 2d8+STR bludgeoning
///   melee. Vanilla SimpleWeapon; no rider effect — the load-bearing
///   identity is the resistance / immunity envelope plus the double-
///   slam multi.
///
/// Defensive identity: AC 9 (no natural armor — soft patchwork flesh),
/// 127 HP (15d8+60). The standard construct envelope plus the **flesh
/// golem signature**: RAW's **Lightning Absorption** — "whenever the
/// golem is subjected to Lightning damage, it regains a number of Hit
/// Points equal to the Lightning damage dealt". It is powered by
/// lightning, and a party that reaches for the obvious answer to a
/// construct puts it back on its feet. Carried as
/// `DamageModifier::Absorption`, which zeroes the damage exactly the
/// way the immunity RAW also prints does, and pays out the heal at the
/// damage chokepoint. Poison immunity (no metabolism), non-magical BPS
/// resistance (the stitched-together hide shrugs off mundane blades).
///
/// Magic Resistance is **off** RAW — RAW doesn't give the flesh golem
/// the construct-tier anti-caster envelope (that's reserved for stone +
/// iron golems further up the ladder). Counter-balanced by the lower
/// HP / AC profile so the golem still goes down to a focused caster
/// barrage at CR 5.
///
/// Condition envelope: the canonical construct immunities — no mind to
/// charm, no joints to freeze, no metabolism to poison or exhaust, no
/// pulse to weather sleep. Petrified joins the set as the standard
/// proxy for the "Immutable Form" clause, matching how Iron / Stone
/// Golems handle the same RAW clause.
///
/// **Berserk** (RAW: each turn the golem starts in damaged state, roll
/// d6 — on a 6 it attacks the nearest creature, friend or foe) is
/// omitted since the engine doesn't model AI-target overrides through
/// the template lane. Future polish bucket if a "berserk_threshold"
/// chassis lands.
///
/// **Aversion to Fire** (RAW: "if the golem takes Fire damage, it has
/// Disadvantage on attack rolls and ability checks until the end of its
/// next turn") is the other half of the golem's elemental character and
/// the exact mirror of the absorption above it — lightning feeds it,
/// fire makes it recoil. It used to be omitted because "the engine
/// doesn't yet have a 'damage-type-triggered debuff' hook"; the hook is
/// `CreatureTemplate::flinches`, and this is one of its two rows.
///
/// Stat shape: AC 9, ~127 HP (15d8+60), STR 19, DEX 9, CON 18, INT 6,
/// WIS 10, CHA 5. Speed 30 (slow lurching shamble). Senses: Darkvision
/// 60. Languages: understands its creator's languages, but doesn't
/// speak — we omit a Language entry to match the "comprehends but
/// doesn't speak" RAW clause cleanly. Size Medium. CR 5.
pub static FLESH_GOLEM_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*FLESH_GOLEM_MULTI);
    actions.push(&FLESH_GOLEM_SLAM);
    CreatureTemplate {
        name: "Flesh Golem",
        // 'F' (uppercase) — distinct from existing 'G' (Stone Golem)
        // and 'I' (Iron Golem) at the same uppercase glyph tier.
        glyph: 'F',
        ac: 9,
        // 15d8+60 ≈ 127 average per MM (CR 5).
        hitpoints: "15d8+60".parse().unwrap(),
        speed: 30.,
        strength: 19,
        intelligence: 6,
        dexterity: 9,
        wisdom: 10,
        constitution: 18,
        charisma: 5,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        cr: 5.0,
        size: Size::Medium,
        creature_type: CreatureType::Construct,
        actions,
        // Construct envelope: non-magical BPS resistance + poison
        // immunity from the shared helper, plus RAW's Lightning
        // Absorption — zero damage taken and an equal number of hit
        // points back.
        damage_modifiers: damage_modifiers_from([
            (DamageType::Lightning, DamageModifier::Absorption),
            (DamageType::Poison, DamageModifier::Immunity),
        ]),
        // RAW **Aversion to Fire**. `Rounds(2)` is the engine's reading
        // of "until the end of its next turn" — see `DamageFlinch`.
        flinches: vec![DamageFlinch {
            types: &[DamageType::Fire],
            condition: Condition::Flinching,
            timer: ConditionTimer::Rounds(2),
            label: "aversion to fire",
        }],
        // Standard construct condition envelope: no mind, no joints, no
        // metabolism, no pulse. Petrified joins the set as the standard
        // proxy for the "Immutable Form" clause (RAW: immune to any
        // effect that would alter its form).
        condition_immunities: HashSet::from([
            Condition::Charmed,
            Condition::Exhausted,
            Condition::Frightened,
            Condition::Paralyzed,
            Condition::Petrified,
            Condition::Poisoned,
        ]),
        ..CreatureTemplate::resistant_to_nonmagical_physical()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::Coordinate;

    #[test]
    fn flesh_golem_template_shape() {
        let a = ActorInstance::from_creature_template(
            &FLESH_GOLEM_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 5.0);
        assert_eq!(a.size(), Size::Medium);
        assert_eq!(a.creature_type(), CreatureType::Construct);
        assert!(a.find_action("flesh golem multiattack").is_some());
        assert!(a.find_action("flesh golem slam").is_some());
    }

    #[test]
    fn flesh_golem_has_construct_envelope() {
        let a = ActorInstance::from_creature_template(
            &FLESH_GOLEM_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        // Lightning absorbed (the golem's life-spark is lightning, and
        // RAW hands it back as hit points), poison immune, non-magical
        // BPS resistant.
        assert_eq!(
            a.damage_modifier(DamageType::Lightning),
            Some(DamageModifier::Absorption)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Poison),
            Some(DamageModifier::Immunity)
        );
        assert_eq!(
            a.nonmagical_damage_modifier(DamageType::Bludgeoning),
            Some(DamageModifier::Resistance)
        );
        // Standard construct condition envelope.
        assert!(a.effectively_immune_to_condition(Condition::Charmed));
        assert!(a.effectively_immune_to_condition(Condition::Frightened));
        assert!(a.effectively_immune_to_condition(Condition::Paralyzed));
        assert!(a.effectively_immune_to_condition(Condition::Petrified));
        assert!(a.effectively_immune_to_condition(Condition::Poisoned));
        // Crucially — flesh golem does NOT have Magic Resistance (RAW
        // reserves it for stone + iron golems further up the ladder).
        assert!(!a.has_magic_resistance());
    }
}
