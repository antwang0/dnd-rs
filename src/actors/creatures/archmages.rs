use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::ARCHMAGE_DAGGER;
use crate::actions::spells::{
    BANISHMENT, CONE_OF_COLD, COUNTERSPELL, FIRE_BOLT, FIREBALL, GLOBE_OF_INVULNERABILITY,
    MAGE_ARMOR, MISTY_STEP, SHIELD, STONESKIN, TIME_STOP,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Archmage — CR 12 medium humanoid. The apex of the NPC appendix and
/// the only entry in it that fights the party the way a party fights.
/// Ninety-nine hit points behind a Shield reaction, Magic Resistance
/// against everything the party casts back, and nine tiers of slots.
///
/// Action lanes, in the order the archmage actually spends them:
/// - **counterspell** — the reaction that makes the fight about the
///   archmage's turn rather than the party's.
/// - **shield** — +5 AC as a reaction. On AC 12 that is the difference
///   between a body and a target.
/// - **time stop** — the level-9 opener, when it has one.
/// - **cone of cold** / **fireball** — the two damage tiers.
/// - **banishment** — one creature removed from the fight entirely.
/// - **globe of invulnerability** / **stoneskin** / **mage armor** —
///   the defensive stack.
/// - **misty step** — the disengage that costs a bonus action.
/// - **fire bolt** — what it does with a turn it has nothing better
///   for.
/// - **archmage dagger** — what it does with a turn it has nothing at
///   all for.
///
/// **Magic Resistance** (`has_magic_resistance`) is advantage on saves
/// against spells, and it is the trait that decides how the fight goes:
/// the party's control spells are the only thing that answers a caster
/// with this many slots, and every one of them now rolls twice.
///
/// Stat shape per the SRD NPC appendix: AC 12 (15 with mage armor), 99
/// HP (18d8+18), STR 10 / DEX 14 / CON 12 / INT 20 / WIS 15 / CHA 16.
/// Speed 30. Proficient INT / WIS saves. Darkvision 60 (RAW's archmage
/// is usually an elf or a half-elf and the appendix prints the sense).
/// CR 12.
///
/// The spell ability is WIS rather than the INT 20 RAW prints, because
/// the engine derives every save DC and spell attack from a single
/// ability and that ability is Wisdom. WIS 15 is set high enough to keep
/// the archmage's DCs in the band its CR expects without pretending the
/// stat block's INT is doing the work.
///
/// The slot ladder is 4/3/3/3/3/2/1/1/1 — RAW's, exactly. It is a lot of
/// rows for a creature that will realistically spend four of them, and
/// that is the point: which four is the archmage's decision, and a
/// truncated ladder would make it the template author's.
pub static ARCHMAGE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*FIRE_BOLT);
    actions.push(&*SHIELD);
    actions.push(&*MAGE_ARMOR);
    actions.push(&*MISTY_STEP);
    actions.push(&*COUNTERSPELL);
    actions.push(&*FIREBALL);
    actions.push(&*BANISHMENT);
    actions.push(&*CONE_OF_COLD);
    actions.push(&*STONESKIN);
    actions.push(&*GLOBE_OF_INVULNERABILITY);
    actions.push(&*TIME_STOP);
    actions.push(&ARCHMAGE_DAGGER);
    CreatureTemplate {
        name: "Archmage",
        // 'm' (lowercase) — the caster band, one rung above the Mage's
        // 'M'. Deliberately the same letter in the other case: the two
        // are the same silhouette at different CRs and the map should
        // say so.
        glyph: 'm',
        ac: 12,
        // 18d8+18 ≈ 99 average per the SRD NPC appendix (CR 12).
        hitpoints: "18d8+18".parse().unwrap(),
        speed: 30.,
        strength: 10,
        dexterity: 14,
        constitution: 12,
        intelligence: 20,
        wisdom: 15,
        charisma: 16,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common, Language::Draconic, Language::Elvish]),
        cr: 12.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        proficient_saves: HashSet::from([AbilityScoreType::Intelligence, AbilityScoreType::Wisdom]),
        spell_slots_by_level: vec![4, 3, 3, 3, 3, 2, 1, 1, 1],
        has_magic_resistance: true,
        // SRD 5.2 "Immunities Psychic; Charmed (with Mind Blank)". The
        // stat block prints the psychic half flat — the archmage keeps a
        // Mind Blank up as a matter of routine — so it lands as a plain
        // immunity rather than as a rider on a spell the engine would
        // have to watch for. The Charmed half is left off for the
        // opposite reason: that one RAW does condition on the spell
        // being up, and there is a real difference between a caster who
        // cannot be charmed and one who has to have spent a slot.
        damage_modifiers: crate::actors::actor_template::damage_modifiers_from([(
            crate::engine::types::DamageType::Psychic,
            crate::engine::types::DamageModifier::Immunity,
        )]),
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
            &ARCHMAGE_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn archmage_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 12.0);
        assert_eq!(a.creature_type(), CreatureType::Humanoid);
        assert!(a.has_magic_resistance());
        assert!(a.find_action("archmage dagger").is_some());
    }

    /// Every rung of the ladder is populated, and the top of it can
    /// actually pay for the spell that sits there. A caster carrying
    /// Time Stop and no level-9 slot is a stat block with a line on it
    /// that never resolves — the failure mode this whole assertion
    /// exists to name.
    #[test]
    fn the_archmage_can_pay_for_every_spell_it_carries() {
        let a = make();
        for (lvl, expected) in (1..=9u32).zip([4, 3, 3, 3, 3, 2, 1, 1, 1]) {
            assert_eq!(
                a.spell_slot_manager.spell_slots(lvl).spell_slots,
                expected,
                "level-{} slots",
                lvl
            );
        }
        assert!(a.find_action("time stop").is_some());
        assert!(a.find_action("counterspell").is_some());
    }
}
