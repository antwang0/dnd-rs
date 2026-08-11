use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    NALFESHNEE_BITE, NALFESHNEE_CLAW, NALFESHNEE_HORROR_NIMBUS, NALFESHNEE_MULTI,
};
use crate::actors::actor_template::{CreatureTemplate, damage_modifiers_from};
use crate::conditions::Condition;
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense,
};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Nalfeshnee — CR 13 large demon (Type V demon). The malformed boar-
/// headed fiend of the Abyss: tusks, claws, an obese wing-grown trunk,
/// and a halo of mind-bending terror. Slots above Hezrou (CR 8) and
/// Glabrezu (CR 9), below Marilith (CR 16) and Balor (CR 19) on the
/// demon hierarchy ladder — the closest analog to a "heavy bruiser
/// with crowd control" at the mid-boss tier.
///
/// Action lanes:
/// - **nalfeshnee multiattack** — 1 bite + 2 claws per Action via the
///   shared `CompoundAttack` chassis. Heterogeneous compound matching
///   the Hezrou / Pit Fiend pattern but tuned to CR 13: 5d10 bite +
///   2×3d6 claws lands around 50 average damage per Action.
/// - **nalfeshnee bite** (standalone) — STR-based 5d10+STR piercing.
/// - **nalfeshnee claw** (standalone) — STR-based 3d6+STR slashing.
/// - **horror nimbus** — Recharge 5–6 area-effect Frighten install.
///   Every creature within 15-ft (3-tile burst) makes a WIS save
///   DC 15 or is Frightened for 10 rounds. Routes through the shared
///   `resolve_burst_save_condition` chokepoint so condition-immune
///   and aura-protected (Aura of Courage) allies shrug it off cleanly.
///
/// Defensive identity: AC 18 (natural armor — the malformed hide),
/// 184 HP (16d10+96). The standard demon damage envelope: resistant
/// to cold + fire + lightning + non-magical BPS (the canonical "demon
/// damage profile" shared with Hezrou / Glabrezu / Marilith). Immune
/// to Poison damage AND the Poisoned condition (demon physiology).
/// Magic Resistance gives advantage on every save vs spells.
///
/// Stat shape: AC 18, ~184 HP (16d10+96), STR 21, DEX 10, CON 22,
/// INT 19, WIS 12, CHA 15. Speed 20 (the demon's bloated frame is
/// slow on land — RAW also grants fly 30 which we don't model). Senses:
/// Truesight 120ft (the demon sees through every illusion / dim-light
/// / invisibility). Languages: Abyssal (the demon tongue). Telepathy
/// 120ft RAW — we drop telepathy since the engine doesn't model it as
/// a language. Size Large. CR 13.
pub static NALFESHNEE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*NALFESHNEE_MULTI);
    actions.push(&NALFESHNEE_BITE);
    actions.push(&NALFESHNEE_CLAW);
    actions.push(&*NALFESHNEE_HORROR_NIMBUS);
    CreatureTemplate {
        name: "Nalfeshnee",
        // 'N' (uppercase) — distinct from existing 'n' (Nothic) and 'M'
        // (Mind Flayer / Manticore / Mage). 'N' for the towering Type-V
        // demon silhouette.
        glyph: 'N',
        ac: 18,
        // 16d10+96 ≈ 184 average per MM (CR 13).
        hitpoints: "16d10+96".parse().unwrap(),
        // RAW speed line: Speed 20 ft., fly 30 ft.
        speed: 20.0,
        fly_speed: 30.0,
        strength: 21,
        intelligence: 19,
        dexterity: 10,
        wisdom: 12,
        constitution: 22,
        charisma: 15,
        // Truesight 120ft — the iconic demon "sees through everything"
        // sensorial envelope. Read by the `countered_by_truesight` cohort
        // so the Nalfeshnee ignores its target's Invisible / Blurred /
        // Displaced disadvantage riders on attack.
        senses: HashSet::from([SpecialSense::Truesight(120)]),
        languages: HashSet::from([Language::Abyssal]),
        cr: 13.0,
        size: Size::Large,
        creature_type: CreatureType::Fiend,
        actions,
        // Nalfeshnee proficient saves: CON, INT, WIS, CHA per MM. The
        // four mental + endurance saves — the demon's malformed body
        // shrugs off shock through sheer durability and willpower.
        proficient_saves: HashSet::from([
            AbilityScoreType::Constitution,
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]),
        // Demon damage envelope: non-magical BPS resistance + cold /
        // fire / lightning resistance + poison immunity. Mirrors the
        // Hezrou / Glabrezu damage profile (same demon family) but on
        // the CR-13 frame.
        damage_modifiers: damage_modifiers_from([
            (DamageType::Cold, DamageModifier::Resistance),
            (DamageType::Fire, DamageModifier::Resistance),
            (DamageType::Lightning, DamageModifier::Resistance),
            (DamageType::Poison, DamageModifier::Immunity),
        ]),
        condition_immunities: HashSet::from([Condition::Poisoned]),
        has_magic_resistance: true,
        // Horror Nimbus is the recharge ability — 5–6 refresh on a d6
        // at start of turn. Routes through the standard recharge pool;
        // the unique `"horror_nimbus"` key keeps the Nalfeshnee's
        // signature from sharing the breath_weapon pool with a co-
        // located dragon.
        recharge_abilities: vec![("horror_nimbus", 5)],
        has_extra_attack: true,
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
    fn nalfeshnee_template_shape() {
        let a = ActorInstance::from_creature_template(
            &NALFESHNEE_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 13.0);
        assert_eq!(a.size(), Size::Large);
        assert_eq!(a.creature_type(), CreatureType::Fiend);
        // The Nalfeshnee's four action lanes — multi primary, bite /
        // claw standalone for AI fallback, plus Horror Nimbus recharge
        // for the signature crowd-control burst.
        assert!(a.find_action("nalfeshnee multiattack").is_some());
        assert!(a.find_action("nalfeshnee bite").is_some());
        assert!(a.find_action("nalfeshnee claw").is_some());
        assert!(a.find_action("horror nimbus").is_some());
    }

    #[test]
    fn nalfeshnee_has_demon_envelope() {
        let a = ActorInstance::from_creature_template(
            &NALFESHNEE_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        // Standard demon resistance envelope — poison immune, BPS / cold
        // / fire / lightning resistant. Mirrors Hezrou / Glabrezu but
        // at CR 13.
        assert_eq!(
            a.damage_modifier(DamageType::Poison),
            Some(DamageModifier::Immunity)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Fire),
            Some(DamageModifier::Resistance)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Cold),
            Some(DamageModifier::Resistance)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Lightning),
            Some(DamageModifier::Resistance)
        );
        assert_eq!(
            a.nonmagical_damage_modifier(DamageType::Slashing),
            Some(DamageModifier::Resistance)
        );
        assert!(a.has_magic_resistance());
        assert!(a.effectively_immune_to_condition(Condition::Poisoned));
    }
}
