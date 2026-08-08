use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    MUMMY_LORD_DREADFUL_GLARE, MUMMY_LORD_MULTI, MUMMY_LORD_ROTTING_FIST,
};
use crate::actors::actor_template::{CreatureTemplate, damage_modifiers_from};
use crate::conditions::Condition;
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Size, SpecialSense,
};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Mummy Lord — CR 15 undead boss. The boss-tier sibling of the basic
/// Mummy: same shape (rotting fist + dreadful glare) on a vastly stronger
/// chassis (3d6+STR + 6d6 necrotic fist, DC-17 60-ft glare), plus Magic
/// Resistance and the Legendary Resistance (3/Day) anti-caster envelope.
/// Boss undead "ancient pharaoh" archetype — slow on foot (20 ft speed RAW)
/// but devastating when the party closes to melee, and the wide-radius
/// glare projects fear pressure on approach.
///
/// Action lanes:
/// - **Mummy Lord Multiattack** — 1 Rotting Fist + 1 Dreadful Glare per
///   Action. The fist swing validates the standard MELEE_REACH reach; the
///   glare fans out from the lord's location regardless. CompoundAttack
///   pattern (mixed-schema multi).
/// - **Lord Rotting Fist** (standalone) — STR-based 3d6+STR bludgeoning
///   plus 6d6 necrotic rider on hit, mirroring the basic mummy's fist at
///   doubled die counts. Use when the lord can't catch a target in the
///   glare cone.
/// - **Lord Dreadful Glare** (standalone) — WIS save DC 17 vs every
///   non-undead enemy within 60 ft (12 tiles) with line-of-sight. Frightens
///   for 10 rounds on fail. Stronger DC and longer reach than the basic
///   mummy's glare; the load-bearing area-control move.
///
/// **Magic Resistance** — advantage on every save vs spells / magical
/// effects. Combined with the 3 Legendary Resistances, the lord is a
/// "anti-caster" boss that shrugs off save-or-suck control spells. The
/// canonical strategy: chip with magical weapons until concentration
/// breaks the LR pool open.
///
/// Damage envelope mirrors the basic mummy template: nonmagical B/P/S
/// resistance, necrotic + poison immunity, fire vulnerability (the
/// wrappings still burn). Condition immunities are the standard undead
/// triplet (Charmed / Frightened / Poisoned) plus Exhausted and Paralyzed
/// per the RAW Mummy Lord stat block. Languages and skills are surfaced
/// only as `Common` since the engine doesn't model skill bonuses.
///
/// RAW also gives the lord Spellcasting (Cleric spell list up to lvl 5)
/// and the **Rejuvenation** clause that revives them from their organs.
/// Both are omitted here — the engine doesn't surface monster spellcasting
/// picks (matches Cloud Giant / Lich's omission) and the rejuvenation
/// hook would require a new revive-from-corpse engine path; the
/// load-bearing per-fight threat is the fist + glare combo plus the LR /
/// MR envelope.
pub static MUMMY_LORD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*MUMMY_LORD_MULTI);
    actions.push(&MUMMY_LORD_ROTTING_FIST);
    actions.push(&*MUMMY_LORD_DREADFUL_GLARE);
    CreatureTemplate {
        name: "Mummy Lord",
        // 'U' is taken by Cloud Giant; the basic mummy uses lowercase 'u'.
        // 'L' (Lord) reads cleanly and is unused in the undead pool.
        glyph: 'L',
        ac: 17,
        // 17d8+85 ≈ 161 average per MM (CR 15).
        hitpoints: "17d8+85".parse().unwrap(),
        speed: 20.,
        strength: 18,
        intelligence: 11,
        dexterity: 10,
        wisdom: 18,
        constitution: 17,
        charisma: 16,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        cr: 15.0,
        size: Size::Medium,
        creature_type: CreatureType::Undead,
        actions,
        // Mummy Lord proficient saves: CON, INT, WIS, CHA per MM.
        proficient_saves: HashSet::from([
            AbilityScoreType::Constitution,
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]),
        damage_modifiers: damage_modifiers_from([
            (DamageType::Necrotic, DamageModifier::Immunity),
            (DamageType::Poison, DamageModifier::Immunity),
            (DamageType::Fire, DamageModifier::Vulnerability),
        ]),
        // Mummy Lord undead immunities: standard undead triplet (Charmed /
        // Frightened / Poisoned) plus Exhausted and Paralyzed per MM.
        condition_immunities: HashSet::from([
            Condition::Charmed,
            Condition::Exhausted,
            Condition::Frightened,
            Condition::Paralyzed,
            Condition::Poisoned,
        ]),
        // 5e Magic Resistance — advantage on every save vs spells /
        // magical effects. Read by `compute_save_mode`.
        has_magic_resistance: true,
        // 5e Legendary Resistance (3/Day) — the lord's anti-caster
        // signature. Three failed saves per long rest are auto-promoted to
        // passes, neutralizing the party's save-or-suck control spells.
        legendary_resistances: 3,
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
    fn mummy_lord_has_boss_undead_envelope() {
        let a = ActorInstance::from_creature_template(
            &MUMMY_LORD_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        // Damage envelope: necrotic + poison immunity, fire vulnerability,
        // physical resistance.
        assert_eq!(
            a.damage_modifier(DamageType::Necrotic),
            Some(DamageModifier::Immunity)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Poison),
            Some(DamageModifier::Immunity)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Fire),
            Some(DamageModifier::Vulnerability)
        );
        assert_eq!(
            a.nonmagical_damage_modifier(DamageType::Slashing),
            Some(DamageModifier::Resistance)
        );
    }

    #[test]
    fn mummy_lord_has_magic_resistance_and_legendary_resistance() {
        let a = ActorInstance::from_creature_template(
            &MUMMY_LORD_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert!(a.has_magic_resistance());
        assert_eq!(a.legendary_resistance_remaining(), 3);
    }

    #[test]
    fn mummy_lord_has_undead_condition_immunities() {
        let a = ActorInstance::from_creature_template(
            &MUMMY_LORD_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert!(a.is_immune_to_condition(Condition::Charmed));
        assert!(a.is_immune_to_condition(Condition::Frightened));
        assert!(a.is_immune_to_condition(Condition::Poisoned));
        assert!(a.is_immune_to_condition(Condition::Paralyzed));
        assert!(a.is_immune_to_condition(Condition::Exhausted));
    }

    #[test]
    fn mummy_lord_has_compound_multiattack_and_components() {
        let a = ActorInstance::from_creature_template(
            &MUMMY_LORD_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert!(a.find_action("lord fist + glare").is_some());
        assert!(a.find_action("lord rotting fist").is_some());
        assert!(a.find_action("lord dreadful glare").is_some());
    }
}
