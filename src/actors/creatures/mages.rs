use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{MAGE_ARCANE_BURST, MAGE_MULTI};
use crate::actions::spells::{
    CONE_OF_COLD, COUNTERSPELL, FIREBALL, FLY, INVISIBILITY, LIGHT, MAGE_ARMOR, MISTY_STEP, SHIELD,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size, Skill};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Mage — CR 6 medium humanoid. The middle rung of the NPC caster
/// ladder: the Acolyte and the Priest below it, the Archmage above.
///
/// It spent a long time as a CR ½ scaffolding stat block — two spell
/// slots, thirteen hit points, and a docstring that described a
/// creature rather than a stat block. That was the right call when the
/// bestiary was four creatures and the engine needed something that
/// cast; it stopped being right the moment the encounter generator
/// started pricing fights by CR, because a creature filed at ½ that the
/// book prices at 6 is a hole in the budget rather than a monster in
/// it. This is the SRD 5.2 stat block.
///
/// Action lanes, in the order the mage actually spends them:
/// - **mage multiattack** — three Arcane Bursts, 3d8 + INT force
///   apiece at 120 ft. RAW's headline change and the whole reason the
///   Mage is a fight: the at-will lane is a CR 6 damage lane that never
///   runs out, so a mage with no slots left is still a problem.
/// - **counterspell** / **shield** — RAW's Protective Magic reaction,
///   3/day for either. The engine has both as ordinary reactions off
///   the slot ladder, which is the same trade at a different price.
/// - **fireball** — RAW casts it at 4th level, 2/day. The engine's
///   Fireball upcasts off whatever slot pays for it, so a mage with a
///   4th-level slot left throws RAW's version and one down to its last
///   3rd throws a smaller one.
/// - **cone of cold** — the 1/day level-5 opener, when it has the slot.
/// - **invisibility** / **fly** / **misty step** — the escape stack,
///   in descending order of what it costs to leave.
/// - **mage armor** — RAW folds this into the printed AC 15 ("included
///   in AC"), and so does this template: `ac: 15` already has it. The
///   action is on the list anyway for the case the buff is stripped.
/// - **light** — the at-will cantrip, and the one thing on the sheet
///   that matters on an unlit board.
///
/// Two RAW clauses are deliberately absent. **Detect Magic**, **Mage
/// Hand** and **Prestidigitation** are at-will utility with no combat
/// surface in this engine. And RAW's per-day counts are translated into
/// the slot ladder below rather than modeled as charges — see there.
///
/// Stat shape per the SRD NPC appendix: AC 15 (mage armor included),
/// 81 HP (18d8), STR 9 / DEX 14 / CON 11 / INT 17 / WIS 12 / CHA 11.
/// Speed 30. Proficient INT / WIS saves. Skills: Arcana, History,
/// Perception. CR 6.
///
/// The spell ability is Intelligence, which is both RAW's and the
/// engine's: `best_spell_save_dc` reads the caster's highest mental
/// score, and on this sheet that is the INT 17 the stat block invests
/// in. The template used to carry a WIS as high as its INT with a
/// comment explaining that the engine could only cast off Wisdom; that
/// has not been true since the DC lane learned to pick, and the
/// inflated Wisdom went with it.
pub static MAGE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*MAGE_MULTI);
    actions.push(&MAGE_ARCANE_BURST);
    actions.push(&*COUNTERSPELL);
    actions.push(&*SHIELD);
    actions.push(&*FIREBALL);
    actions.push(&*CONE_OF_COLD);
    actions.push(&*INVISIBILITY);
    actions.push(&*FLY);
    actions.push(&*MISTY_STEP);
    actions.push(&*MAGE_ARMOR);
    actions.push(&*LIGHT);
    CreatureTemplate {
        name: "Mage",
        // 'M' — the caster band, one rung below the Archmage's 'm'.
        // Deliberately the same letter in the other case: the two are
        // the same silhouette at different CRs and the map should say
        // so.
        glyph: 'M',
        // RAW prints 15 with the note "mage armor included in AC", so
        // the number already has the spell in it.
        ac: 15,
        // 18d8 = 81 average per the SRD NPC appendix (CR 6).
        hitpoints: "18d8".parse().unwrap(),
        speed: 30.,
        strength: 9,
        dexterity: 14,
        constitution: 11,
        intelligence: 17,
        wisdom: 12,
        charisma: 11,
        skills: HashSet::from([Skill::Arcana, Skill::History, Skill::Perception]),
        languages: HashSet::from([Language::Common, Language::Draconic, Language::Elvish]),
        cr: 6.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        proficient_saves: HashSet::from([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
        ]),
        // RAW prices this stat block in per-day charges rather than in
        // slots: Fireball (level 4) and Invisibility 2/day each, Cone of
        // Cold and Fly 1/day each, Misty Step 3/day, Protective Magic
        // (Counterspell or Shield) 3/day. The engine has one currency
        // for all of that, so the ladder below is those counts read back
        // into the tier each spell is cast from — three 1st for Shield,
        // three 2nd for Misty Step and Invisibility, two 3rd for Fly and
        // Counterspell, two 4th for RAW's upcast Fireball, one 5th for
        // Cone of Cold.
        //
        // It is a translation rather than a transcription and it errs
        // generous in one place: RAW's Shield is a reaction with its own
        // charge pool, so a mage that spends its 1st-level slots
        // elsewhere loses a defence RAW would have kept. The alternative
        // — a bespoke charge pool per spell — would be a second
        // resource system for one stat block.
        spell_slots_by_level: vec![3, 3, 2, 2, 1],
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
            &MAGE_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn mage_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 6.0);
        assert_eq!(a.creature_type(), CreatureType::Humanoid);
        assert!(a.find_action("mage multiattack").is_some());
        assert!(a.find_action("arcane burst").is_some());
        assert!(a.find_action("fireball").is_some());
    }

    /// The at-will lane is the point of the stat block, and it survives
    /// an empty slot ladder.
    ///
    /// RAW's Mage is dangerous because Arcane Burst costs nothing: the
    /// 2014 printing gave it a dagger, and a mage the party had drained
    /// was a mage the party could walk past. Asserted on a creature
    /// with every slot spent, because that is the state the clause
    /// exists for.
    #[test]
    fn a_mage_out_of_slots_still_has_something_to_throw() {
        let mut a = make();
        for level in 1..=5u32 {
            while a.spell_slot_manager.consume_spell_slot(level) {}
        }
        assert!(
            a.find_action("mage multiattack").is_some(),
            "the burst lane does not spend slots"
        );
    }

    /// The mage casts off Intelligence, not off whatever the engine
    /// used to be able to read.
    ///
    /// Worth pinning because the old template inflated Wisdom to 14 —
    /// level with its Intelligence — purely to route around a save-DC
    /// lane that could only read one ability. That lane picks the
    /// highest mental score now, and a stat block that still carried
    /// the workaround would be casting at the same DC for the wrong
    /// reason.
    #[test]
    fn the_mage_casts_off_the_ability_its_stat_block_invests_in() {
        let a = make();
        assert_eq!(
            a.best_spellcasting_ability(ActorInstance::SPELLCASTING_ABILITIES),
            AbilityScoreType::Intelligence
        );
        assert!(
            a.ability_score(AbilityScoreType::Intelligence)
                > a.ability_score(AbilityScoreType::Wisdom),
            "the Wisdom crutch is gone"
        );
    }
}
