use crate::actions::class_features::TURN_RESISTANCE_TAG;
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    MUMMY_LORD_DREADFUL_GLARE, MUMMY_LORD_MULTI, MUMMY_LORD_ROTTING_FIST,
};
use crate::actors::actor_template::{CreatureTemplate, damage_modifiers_from};
use crate::conditions::Condition;
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Size, Skill, SpecialSense,
};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Mummy Lord — CR 15 undead boss. The boss-tier sibling of the basic
/// Mummy: same shape (rotting fist + dreadful glare) on a vastly stronger
/// chassis (3d6+STR + 6d6 necrotic fist, DC-17 60-ft glare), plus Magic
/// Resistance and the Legendary Resistance (3/Day) anti-caster envelope.
/// Boss undead "ancient pharaoh" archetype — unhurried on foot (30 ft)
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
/// **Truesight 60 ft**, which is what SRD 5.2 prints and what an
/// ancient pharaoh with a party of illusionists in the room is for. The
/// block carried Darkvision 60 — the sense one rung down, and the one
/// a Greater Invisibility walks straight past.
///
/// **Spellcasting** — *"the mummy casts one of the following spells,
/// requiring no Material components and using Wisdom as the
/// spellcasting ability (spell save DC 17, +9 to hit with spell
/// attacks): At Will: Dispel Magic, Thaumaturgy. 1/Day Each: Animate
/// Dead, Harm, Insect Plague (level 7 version)."*
///
/// It used to be omitted, under a docstring saying *"the engine doesn't
/// surface monster spellcasting picks (matches Cloud Giant / Lich's
/// omission)"*. The lich two files over casts twenty-one spells off a
/// nine-row slot table, and has for a long time; the claim outlived the
/// engine it described by a wide margin. What it cost was the one thing
/// this stat block is otherwise missing — a lord whose fist and glare
/// both want the party close had nothing at all to do about a caster
/// standing sixty feet away behind a Wall of Force.
///
/// Wisdom is picked up for free: `best_spellcasting_ability` takes the
/// best mental score and the lord's is WIS 19 against INT 11 and CHA
/// 16, so the DC and the attack bonus land on RAW's number without a
/// per-monster override.
///
/// **The slot table is RAW's daily list, priced.** *1/Day Each* is one
/// casting per long rest, and a long rest is what separates two fights
/// in this engine, so one slot at each of levels 4, 6 and 7 buys
/// exactly Animate Dead, Harm and a level-7 Insect Plague, once each,
/// per encounter. The at-will half is the part that cannot be said
/// exactly: three level-3 slots stand in for unlimited Dispel Magic,
/// because *"at will"* has no expression in a slot table and an
/// unlimited counter-to-every-buff is not a thing to approximate
/// upward. Thaumaturgy is left out — it is a cantrip whose whole text
/// is stage effects.
///
/// The **Rejuvenation** clause that revives the lord from its organs is
/// still omitted: it needs a revive-from-corpse path nothing in the
/// engine has, and a heart on the floor that the party has to find is a
/// scenario rather than a combat mechanic.
pub static MUMMY_LORD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*MUMMY_LORD_MULTI);
    actions.push(&MUMMY_LORD_ROTTING_FIST);
    actions.push(&*MUMMY_LORD_DREADFUL_GLARE);
    // SRD 5.2 Spellcasting, in the book's own order: the at-will entry
    // first, then the three the lord gets once a day each. See the
    // docstring for how "At Will" and "1/Day Each" are priced into the
    // slot table below.
    actions.push(&*crate::actions::spells::DISPEL_MAGIC);
    actions.push(&crate::actions::spells::ANIMATE_DEAD);
    actions.push(&*crate::actions::spells::HARM);
    actions.push(&*crate::actions::spells::INSECT_PLAGUE);
    CreatureTemplate {
        name: "Mummy Lord",
        // 'U' is taken by Cloud Giant; the basic mummy uses lowercase 'u'.
        // 'L' (Lord) reads cleanly and is unused in the undead pool.
        glyph: 'L',
        ac: 17,
        // 25d8+75 ≈ 187 average per MM (CR 15).
        hitpoints: "25d8+75".parse().unwrap(),
        speed: 30.,
        strength: 18,
        intelligence: 11,
        dexterity: 10,
        wisdom: 19,
        constitution: 17,
        charisma: 16,
        senses: HashSet::from([SpecialSense::Truesight(60)]),
        // RAW's daily list, priced into slots — three level-3 castings
        // standing in for at-will Dispel Magic, and one each at 4, 6 and
        // 7 for Animate Dead, Harm and the level-7 Insect Plague. Levels
        // 1, 2 and 5 are empty because the lord knows nothing there; a
        // slot with no spell behind it is a slot the AI walks past.
        spell_slots_by_level: vec![0, 0, 3, 1, 0, 1, 1],
        cr: 15.0,
        size: Size::Medium,
        creature_type: CreatureType::Undead,
        actions,
        // SRD 5.2 **Turn Resistance** — a cleric's Channel Divinity is
        // a poor answer to this one. See `TURN_RESISTANCE_TAG`.
        features: HashSet::from([TURN_RESISTANCE_TAG]),
        // SRD 5.2: *"Int +5 … Wis +9"*. The CON and CHA proficiencies
        // beside them were the 2014 stat block's; what keeps this
        // thing standing now is the Turn Resistance above.
        proficient_saves: HashSet::from([AbilityScoreType::Intelligence, AbilityScoreType::Wisdom]),
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
        skills: HashSet::from([Skill::History, Skill::Perception, Skill::Religion]),
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

#[cfg(test)]
mod spell_tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::Coordinate;

    fn a_lord() -> ActorInstance {
        ActorInstance::from_creature_template(
            &MUMMY_LORD_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    /// SRD 5.2's Spellcasting line, as four actions and a slot table.
    ///
    /// The list is asserted by name because the omission it replaces was
    /// invisible: a stat block missing a whole trait looks exactly like
    /// one that never had it, and the only thing that ever said
    /// otherwise was a docstring explaining why it could not be there.
    #[test]
    fn the_lord_casts_the_four_spells_its_stat_block_prints() {
        let a = a_lord();
        for spell in ["dispel magic", "animate dead", "harm", "insect plague"] {
            assert!(
                a.find_action(spell).is_some(),
                "SRD 5.2 prints {} on the Mummy Lord and the block does not carry it",
                spell
            );
        }
    }

    /// *"Using Wisdom as the spellcasting ability (spell save DC 17, +9
    /// to hit with spell attacks)."*
    ///
    /// Both of RAW's numbers, and neither is written down anywhere in
    /// this file: the DC falls out of WIS 19 and a CR-15 proficiency
    /// bonus of +5, and the ability falls out of `best_spellcasting_ability`
    /// preferring the lord's best mental score. The test exists because
    /// that is a coincidence worth pinning — a template that grew a
    /// higher Charisma would quietly start casting at the wrong number.
    #[test]
    fn the_lord_casts_off_wisdom_at_the_dc_its_block_prints() {
        let a = a_lord();
        assert_eq!(
            a.best_spellcasting_ability([
                AbilityScoreType::Intelligence,
                AbilityScoreType::Wisdom,
                AbilityScoreType::Charisma,
            ]),
            AbilityScoreType::Wisdom,
            "an ancient priest casts off Wisdom"
        );
        assert_eq!(
            a.spell_save_dc(AbilityScoreType::Wisdom),
            17,
            "RAW prints DC 17"
        );
        assert_eq!(
            a.spell_attack_modifier(AbilityScoreType::Wisdom),
            9,
            "and +9 to hit with spell attacks"
        );
    }

    /// *"1/Day Each"* — one Animate Dead, one Harm, one level-7 Insect
    /// Plague per rest, and nothing at levels the lord knows no spell
    /// for.
    ///
    /// The empty rows are the half worth asserting: a slot with no spell
    /// behind it is a resource the AI would price into its decisions and
    /// never be able to spend, and a boss with a phantom level-1 pool
    /// reads as a caster it is not.
    #[test]
    fn the_daily_list_is_one_slot_each_and_nothing_spare() {
        let a = a_lord();
        for (lvl, want) in [(1, 0), (2, 0), (3, 3), (4, 1), (5, 0), (6, 1), (7, 1), (8, 0)] {
            assert_eq!(
                a.spell_slot_manager.spell_slots(lvl).max_spell_slots,
                want,
                "level-{} pool",
                lvl
            );
        }
    }

    /// *"Senses Truesight 60 ft."* — the sense the block had one rung
    /// too low.
    #[test]
    fn the_lord_sees_truly() {
        let a = a_lord();
        assert!(
            a.senses().contains(&SpecialSense::Truesight(60)),
            "SRD 5.2 gives the lord Truesight, not Darkvision"
        );
        assert!(
            !a.senses().contains(&SpecialSense::Darkvision(60)),
            "and it is a replacement rather than an addition"
        );
    }
}
