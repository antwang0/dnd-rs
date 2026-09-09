use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    ANDROSPHINX_CLAW, ANDROSPHINX_MULTI, ANDROSPHINX_ROAR,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{
    AbilityScoreType, CreatureType, Language, Size, SpecialSense,
};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Androsphinx — CR 17 large monstrosity boss. The lion-bodied, beak-faced
/// guardian of ancient temples and forbidden knowledge. Slots opposite
/// Adult Red Dragon / Dragon Turtle (both CR 17) on the boss ladder —
/// the "wisdom guardian" lane to the dragons' "elemental tyrant" lane.
/// Load-bearing per-round threat is the double-claw multi (heavy CR-17
/// physical damage band) plus the Roar, whose three uses escalate from
/// a room-wide fear to a room-wide paralysis to eight d10s of thunder.
///
/// Action lanes:
/// - **Androsphinx Multiattack** — 2 claw swings per Action. Vanilla
///   single-sub-attack shape mirroring Hook Horror / Rakshasa multis.
/// - **Androsphinx Claw** (standalone) — STR-based 2d8+STR slashing.
///   Provided so the AI can fall back to a single swing when bonus-
///   action-tagged or moving in.
/// - **Roar** — three a day, and a different roar each time: fear,
///   then a paralysis its victims roll their way out of, then eight
///   d10s of thunder that puts whoever is left on the floor. The whole
///   escalation ships; see `ANDROSPHINX_ROAR`. It is what makes this
///   stat block different from every other boss on the roster, because
///   the answer to "what does the sphinx do next" changes as the fight
///   goes on.
///
/// Damage envelope: nonmagical B/P/S resistance (the canonical "magic
/// weapons or nothing" boss defense). RAW pairs this with the sphinx's
/// own magical weapons clause; this engine doesn't track attacker-side
/// magic-weapon typing, so the resistance effectively only applies to
/// PC weapon swings without a magic-weapon flag.
///
/// Condition immunities: Charmed, Frightened (the canonical "sphinx is
/// a guardian, not a thrall" envelope shared with Ancient Red Dragon).
///
/// Stat shape: AC 17 (natural armor), ~199 average HP (19d10+95), STR 22,
/// WIS 23, CHA 18 — the high-WIS / high-CHA guardian statline. Senses:
/// Truesight 120 ft (the canonical "sees through illusion" guardian
/// envelope — slots into the `TrueSighted` condition cohort's senses
/// equivalent). Languages: Common, Sphinx (we use Sylvan as the closest
/// engine analog since Sphinx isn't enumerated). CR 17.
///
/// **Legendary Resistance (3/Day)** + **Magic Resistance** — the boss
/// anti-caster envelope. Failed saves auto-promote to passes 3 times per
/// long rest; the broader save-advantage envelope hardens the sphinx
/// against the party's save-or-suck spells. Legendary Actions: 3 per
/// round (the engine surfaces the resource via
/// `legendary_actions_per_round` but the load-bearing combat clause is
/// the LR + MR + multi + Roar combo, not the legendary-action lane).
///
/// RAW also gives the sphinx Innate Spellcasting (Cleric-flavored: Detect
/// Magic / Dispel Magic / Plane Shift / Greater Restoration / etc.) plus
/// the Inscrutable trait (immune to thought-reading / emotion-sense
/// effects). Omitted here per the same convention as Rakshasa / Lich /
/// Mummy Lord — the engine doesn't surface monster spellcasting picks,
/// and the load-bearing per-round threat is already the multi + Roar
/// chassis. The "inscrutable" clause has no in-engine consumer (no
/// thought-reading actions surface emotion / mind state).
pub static ANDROSPHINX_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*ANDROSPHINX_MULTI);
    actions.push(&ANDROSPHINX_CLAW);
    actions.push(&*ANDROSPHINX_ROAR);
    CreatureTemplate {
        name: "Androsphinx",
        // 'S' (uppercase) — distinct from 's' (Stirge / Skeleton) and
        // unused in the monstrosity pool. Mnemonic: the regal seated
        // sphinx silhouette.
        glyph: 'S',
        ac: 17,
        // 19d10+95 ≈ 199 average per MM (CR 17).
        hitpoints: "19d10+95".parse().unwrap(),
        // RAW speed line: Speed 40 ft., fly 60 ft.
        speed: 40.0,
        fly_speed: 60.0,
        strength: 22,
        intelligence: 16,
        dexterity: 10,
        wisdom: 23,
        constitution: 20,
        charisma: 18,
        senses: HashSet::from([SpecialSense::Truesight(120)]),
        // RAW languages: Common + "Sphinx". Sphinx isn't enumerated in the
        // engine's `Language` set, so we substitute Sylvan as the closest
        // analog (fey + guardian register).
        languages: HashSet::from([Language::Common, Language::Sylvan]),
        cr: 17.0,
        size: Size::Large,
        creature_type: CreatureType::Monstrosity,
        actions,
        // Nonmagical B/P/S resistance — the boss "magic weapons or
        // nothing" envelope. No damage-type resistances beyond physical;
        // RAW gives the androsphinx no elemental resistance lane.
        // Sphinx is a guardian, not a thrall: immune to Charmed and
        // Frightened (the canonical boss-tier social-debuff envelope).
        condition_immunities: HashSet::from([
            Condition::Charmed,
            Condition::Frightened,
        ]),
        // RAW proficient saves: DEX, CON, INT, WIS — the broad
        // "wisdom guardian" save profile. Folded into the standard
        // proficient_saves lane so `roll_save` adds the proficiency
        // bonus on each save the sphinx rolls.
        proficient_saves: HashSet::from([
            AbilityScoreType::Dexterity,
            AbilityScoreType::Constitution,
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
        ]),
        // 5e Legendary Resistance (3/Day) + Magic Resistance — the
        // anti-caster envelope shared with Ancient Red Dragon / Lich /
        // Pit Fiend at the boss tier.
        legendary_resistances: 3,
        has_magic_resistance: true,
        // No recharge. SRD 5.2 rations the Roar at 3/Day and makes
        // each of the three a *different* roar, which a recharge pool
        // cannot say — see `ROAR_TAG`, whose count is the sequence.
        // 5e Legendary Actions — 3 per round per MM. The engine
        // surfaces the resource via `legendary_actions_per_round` but
        // no Action in the engine currently costs the legendary slot
        // (the Resource::LegendaryAction lane is wired but unused);
        // the bookkeeping is kept for forward compatibility.
        legendary_actions_per_round: 3,
        legendary_actions: crate::engine::legendary_actions::ANDROSPHINX_LEGENDARY,
        has_extra_attack: true,
        // 5e **Magic Weapons**: "the sphinx's weapon attacks are
        // magical", plus the Roar's three-a-day pool.
        features: HashSet::from([
            crate::actions::class_features::MAGICAL_ATTACKS_TAG,
            crate::actions::class_features::ROAR_TAG,
        ]),
        ..CreatureTemplate::resistant_to_nonmagical_physical()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::{Coordinate, DamageModifier, DamageType};

    #[test]
    fn androsphinx_has_multi_and_roar() {
        let a = ActorInstance::from_creature_template(
            &ANDROSPHINX_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert!(a.find_action("androsphinx multiattack").is_some());
        assert!(a.find_action("androsphinx claw").is_some());
        assert!(a.find_action("roar").is_some());
        assert_eq!(a.cr(), 17.0);
        assert_eq!(a.size(), Size::Large);
        assert!(a.has_extra_attack());
    }

    #[test]
    fn androsphinx_has_boss_defensive_envelope() {
        let a = ActorInstance::from_creature_template(
            &ANDROSPHINX_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        // Nonmagical B/P/S resistance — the boss "magic weapons or
        // nothing" envelope.
        assert_eq!(
            a.nonmagical_damage_modifier(DamageType::Bludgeoning),
            Some(DamageModifier::Resistance)
        );
        assert_eq!(
            a.nonmagical_damage_modifier(DamageType::Piercing),
            Some(DamageModifier::Resistance)
        );
        assert_eq!(
            a.nonmagical_damage_modifier(DamageType::Slashing),
            Some(DamageModifier::Resistance)
        );
        // Guardian envelope: immune to charm + frighten.
        assert!(a.effectively_immune_to_condition(Condition::Charmed));
        assert!(a.effectively_immune_to_condition(Condition::Frightened));
        // Boss anti-caster: 3 LR + Magic Resistance.
        assert_eq!(a.legendary_resistance_remaining(), 3);
        assert!(a.has_magic_resistance());
        // 3 legendary actions per round.
        assert_eq!(a.legendary_actions_per_round(), 3);
    }
}
