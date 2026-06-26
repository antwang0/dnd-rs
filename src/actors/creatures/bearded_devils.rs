use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    BEARDED_DEVIL_BEARD, BEARDED_DEVIL_GLAIVE, BEARDED_DEVIL_MULTI,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{
    CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense,
};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Bearded Devil (Barbazu) — CR 3 medium fiend (lawful-evil devil). The
/// frontline polearm devil of the Nine Hells. Slots between Wererat
/// (CR 2) and Werewolf / Bearded Devil-tier (CR 3) on the fiend ladder
/// — above Quasit / Imp (CR 1) and the manes-tier Dretch (CR ¼), below
/// Cambion (CR 5) and the higher devils (Pit Fiend, Ice Devil). The
/// barbazu's signature is the glaive + venomous beard multi: the
/// glaive at reach 10ft, the beard at reach 5ft with a Poisoned
/// condition rider that locks the target's HP regen.
///
/// Action lanes:
/// - **bearded devil multiattack** — 1 glaive + 1 beard per Action via
///   `CompoundAttack`. Heterogeneous compound (slashing + piercing) —
///   the beard carries the Poisoned rider, the glaive is the steady
///   damage lane. Same shape as the wereXX bite + claws compound but
///   tuned to CR 3.
/// - **bearded devil glaive** (standalone) — STR-based 1d10+STR
///   slashing melee at reach 10ft. The polearm-style primary lane.
/// - **bearded devil beard** (standalone) — STR-based 1d8+STR piercing
///   melee at reach 5ft with a CON 12 save rider for Poisoned (3
///   rounds). RAW's "no-healing-while-poisoned" clause is approximated
///   by leaning on the engine's standard Poisoned condition (which
///   imposes disadvantage on attacks / ability checks); the no-healing
///   clause is dropped since healing isn't a tactically-load-bearing
///   axis in this combat sim.
///
/// Defensive identity: resistant to cold (the devil family), immune to
/// fire / poison (the canonical hellish damage envelope). Immune to
/// Poisoned condition (the devil's mind-and-body envelope). Magic
/// Resistance (advantage on saves vs spells), the standard caster-
/// counter lane shared by the mid-tier fiends. The "non-magical
/// weapons" RAW resistance clause is omitted since the engine doesn't
/// tag attacks magical/mundane. RAW also gives Steadfast (immune to
/// being Frightened while in line-of-sight of an allied devil) —
/// we approximate as a flat Frightened immunity since the engine
/// doesn't model "see-an-ally" gating cleanly.
///
/// Stat shape: AC 13 (natural armor — the hellish hide), 52 HP (8d8+16),
/// STR 16, DEX 15, CON 15, INT 9, WIS 11, CHA 11. Speed 30 (the
/// barbazu's confident march). Senses: Darkvision 120. Languages:
/// Infernal (the devil tongue), with telepathy 120ft — we drop
/// telepathy since the engine doesn't model it as a language. Size
/// Medium. CR 3.
pub static BEARDED_DEVIL_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*BEARDED_DEVIL_MULTI);
    actions.push(&BEARDED_DEVIL_GLAIVE);
    actions.push(&BEARDED_DEVIL_BEARD);
    CreatureTemplate {
        name: "Bearded Devil",
        // 'B' (uppercase) — distinct from 'b' (Bandit lowercase). Shared
        // with Balor / Bone Devil / Bullette / Bullywug in the upper-
        // case glyph pool — the bearded devil's CR-3 reach-polearm
        // silhouette stands apart from the CR-19 balor on the encounter
        // map (CR gap surfaces in the encounter generator's pool).
        glyph: 'B',
        ac: 13,
        // 8d8+16 ≈ 52 average per MM (CR 3).
        hitpoints: "8d8+16".parse().unwrap(),
        speed: 30.,
        strength: 16,
        intelligence: 9,
        dexterity: 15,
        wisdom: 11,
        constitution: 15,
        charisma: 11,
        senses: HashSet::from([SpecialSense::Darkvision(120)]),
        languages: HashSet::from([Language::Infernal]),
        cr: 3.0,
        size: Size::Medium,
        creature_type: CreatureType::Fiend,
        actions,
        damage_modifiers: HashMap::from([
            (DamageType::Cold, DamageModifier::Resistance),
            (DamageType::Fire, DamageModifier::Immunity),
            (DamageType::Poison, DamageModifier::Immunity),
        ]),
        // Devil envelope condition immunities — Poisoned is the load-
        // bearing one (poison-immune already short-circuits damage-
        // typed Poisoned installs but condition-only paths still route
        // through here). Steadfast → flat Frightened immunity since
        // the engine doesn't model "see-an-ally" gating cleanly.
        condition_immunities: HashSet::from([Condition::Poisoned, Condition::Frightened]),
        // Magic Resistance: advantage on saves vs spells. Standard
        // mid-tier fiend trait.
        has_magic_resistance: true,
        ..CreatureTemplate::defaults()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::Coordinate;

    #[test]
    fn bearded_devil_template_shape() {
        let a = ActorInstance::from_creature_template(
            &BEARDED_DEVIL_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 3.0);
        assert_eq!(a.size(), Size::Medium);
        assert_eq!(a.creature_type(), CreatureType::Fiend);
        // The barbazu's three action lanes — multi (glaive + beard)
        // primary, glaive + beard standalone for AI fallback.
        assert!(a.find_action("bearded devil multiattack").is_some());
        assert!(a.find_action("bearded devil glaive").is_some());
        assert!(a.find_action("bearded devil beard").is_some());
    }

    #[test]
    fn bearded_devil_has_devil_envelope() {
        let a = ActorInstance::from_creature_template(
            &BEARDED_DEVIL_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        // Devil envelope: fire / poison immune, cold resistant, magic
        // resistance flag, mind-affecting condition immunities.
        assert_eq!(
            a.damage_modifier(DamageType::Fire),
            Some(DamageModifier::Immunity)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Poison),
            Some(DamageModifier::Immunity)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Cold),
            Some(DamageModifier::Resistance)
        );
        assert!(a.has_magic_resistance());
        assert!(a.effectively_immune_to_condition(Condition::Poisoned));
        assert!(a.effectively_immune_to_condition(Condition::Frightened));
    }
}
