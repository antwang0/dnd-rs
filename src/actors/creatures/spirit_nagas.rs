use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::SPIRIT_NAGA_BITE;
use crate::actions::spells::{CHARM_PERSON, HOLD_PERSON, LIGHTNING_BOLT, SACRED_FLAME, SLEEP};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense,
};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Spirit Naga — CR 8 large monstrosity. Snake-bodied caster that punishes
/// parties with both a save-or-suck bite (7d8 poison on a fail) and a
/// modest spell slate (Sleep, Charm Person, Hold Person, Lightning Bolt,
/// Sacred Flame). Slots between Drider (CR 6) and Cloud Giant (CR 9) on
/// the mid-CR ladder — the only spellcasting monstrosity in the pool until
/// the Androsphinx's apex slot opens at CR 17.
///
/// Action lanes:
/// - **naga bite** — STR-based 1d6+STR piercing at reach 2 (10 ft RAW)
///   with a 7d8 poison save rider (CON 13, half on save). The poison
///   rider is the load-bearing per-round threat — the d6 base hit is
///   almost cosmetic next to the 7d8 average (~31) poison packet.
/// - **sleep** — level-1 control (5d8 HP-pool put to sleep). Pairs with
///   the bite as the soft-control opener.
/// - **charm person** — level-1 single-target control.
/// - **hold person** — level-2 single-target paralysis.
/// - **lightning bolt** — level-3 line AoE (8d6 lightning). The naga's
///   apex burst — pairs with the bite's poison rider for a brutal opener.
/// - **sacred flame** — level-0 at-will cantrip; the naga's fallback when
///   spell slots run dry.
///
/// Defensive identity: AC 17 (natural armor — the naga's scaled hide).
/// Poison damage immunity + Poisoned condition immunity — the naga is
/// venom incarnate; her own kind's bite has no effect. Charmed condition
/// immunity — the naga's mind is too alien to coerce.
///
/// Stat shape: AC 17, ~135 HP (18d10+36), STR 18, DEX 17, CON 14, INT 16,
/// WIS 15, CHA 16. Speed 40 (the naga's long coiled body propels her
/// forward faster than a standard medium humanoid). Senses: Darkvision
/// 60. Languages: Abyssal, Common. Saving Throws DEX, CON, WIS, CHA all
/// proficient — the naga is a hardened saver matching her demonic origin.
/// Size Large. CR 8.
///
/// RAW also gives the spirit naga **Rejuvenation** (returns to life 1d10
/// days after being slain, unless its phylactery / soul-anchor is also
/// destroyed) — no in-engine consumer (the engine doesn't model
/// post-combat resurrection timers). The signature load-bearing
/// resurrection clause is the spirit naga's "you can never permakill
/// her" tag, which we approximate by leaving the standard Dead-on-zero
/// transition in place: the player just kills her again in the rematch.
pub static SPIRIT_NAGA_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*SPIRIT_NAGA_BITE);
    // Cantrip — at-will fallback when the slot bag is empty.
    actions.push(&*SACRED_FLAME);
    // Level 1 — soft control openers.
    actions.push(&*SLEEP);
    actions.push(&*CHARM_PERSON);
    // Level 2 — Hold Person single-target paralysis.
    actions.push(&*HOLD_PERSON);
    // Level 3 — Lightning Bolt line-burst, the naga's apex AoE.
    actions.push(&*LIGHTNING_BOLT);
    CreatureTemplate {
        name: "Spirit Naga",
        // 'n' (lowercase) — distinct from 'N' (Night Hag / Nightmare both
        // use 'N' uppercase). Reads as the smaller-statured medium snake
        // silhouette even though the naga is Large; the glyph pool is
        // exhausted at letters that still read as "serpent".
        glyph: 'n',
        ac: 17,
        // 18d10+36 ≈ 135 average per MM (CR 8).
        hitpoints: "18d10+36".parse().unwrap(),
        speed: 40.,
        strength: 18,
        intelligence: 16,
        dexterity: 17,
        wisdom: 15,
        constitution: 14,
        charisma: 16,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Abyssal, Language::Common]),
        cr: 8.0,
        size: Size::Large,
        creature_type: CreatureType::Monstrosity,
        actions,
        // 5e Spirit Naga RAW save proficiencies: DEX +6, CON +5, WIS +5,
        // CHA +6. We model the full save proficiency envelope so the
        // naga's hardened saver identity comes through (otherwise her
        // CR-8 stat block would lose 3-4 points off every non-physical
        // save against a CR-appropriate party caster).
        proficient_saves: HashSet::from([
            AbilityScoreType::Dexterity,
            AbilityScoreType::Constitution,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]),
        // MM Spirit Naga: immune to poison damage (her own venom flavor).
        damage_modifiers: HashMap::from([(DamageType::Poison, DamageModifier::Immunity)]),
        // MM Spirit Naga condition immunities: Charmed, Poisoned. The
        // Charmed lane stems from "her mind is too alien to coerce"; the
        // Poisoned lane matches the poison-damage immunity flavor.
        condition_immunities: HashSet::from([Condition::Charmed, Condition::Poisoned]),
        // Spell slots: 4 / 3 / 3 — matches RAW (5e Spirit Naga is a
        // 10th-level caster with 4 level-1, 3 level-2, 3 level-3 slots,
        // plus 3 level-4 and 2 level-5 slots that we omit since the
        // engine's naga spell pick doesn't go past level 3). Spend
        // priority is Lightning Bolt (lv 3) first, then Hold Person
        // (lv 2), then Sleep / Charm Person (lv 1) — the AI's
        // higher-cost-spell-first heuristic should pick them in
        // descending order naturally.
        spell_slots_by_level: vec![4, 3, 3],
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
    fn spirit_naga_template_shape() {
        let a = ActorInstance::from_creature_template(
            &SPIRIT_NAGA_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 8.0);
        assert_eq!(a.size(), Size::Large);
        assert_eq!(a.creature_type(), CreatureType::Monstrosity);
        // The naga's primary attack lane.
        assert!(a.find_action("naga bite").is_some());
        // The spell list — every entry in the casting roster should land
        // on the instance's action list.
        assert!(a.find_action("sleep").is_some());
        assert!(a.find_action("charm person").is_some());
        assert!(a.find_action("hold person").is_some());
        assert!(a.find_action("lightning bolt").is_some());
        assert!(a.find_action("sacred flame").is_some());
    }

    #[test]
    fn spirit_naga_has_poison_envelope() {
        let a = ActorInstance::from_creature_template(
            &SPIRIT_NAGA_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        // Poison damage immunity — the naga is venom incarnate.
        assert_eq!(
            a.damage_modifier(DamageType::Poison),
            Some(DamageModifier::Immunity)
        );
        // Poisoned + Charmed condition immunity.
        assert!(a.effectively_immune_to_condition(Condition::Poisoned));
        assert!(a.effectively_immune_to_condition(Condition::Charmed));
    }
}
