use crate::actions::class_features::{ACTION_SURGE, ACTION_SURGE_TAG, SECOND_WIND, SECOND_WIND_TAG};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{LONGBOW, LONGSWORD, SCIMITAR, SHORTBOW, SHORTSWORD};
use crate::actions::spells::{
    DANCING_LIGHTS, DARKNESS, FAERIE_FIRE, LONGSTRIDER, MISTY_STEP, PASS_WITHOUT_TRACE,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{
    AbilityScoreType, CreatureType, Language, Size, Skill, SpecialSense,
};
use std::collections::HashSet;
use std::sync::LazyLock;

/// The shared Elf chassis, with no lineage chosen.
///
/// SRD 5.2's Elf is four traits every elf has and one table of three
/// they choose from, which is the same shape the Goliath's Giant
/// Ancestry has and gets the same treatment: a private builder, three
/// public templates, and nothing playable in between. RAW makes the
/// lineage a required choice at character creation, so an elf without
/// one is half a species.
///
/// **The four shared traits, and where each lands.**
///
/// | trait | RAW | engine surface |
/// |-------|-----|----------------|
/// | Darkvision | 60 ft (120 for a drow) | `senses` |
/// | Fey Ancestry | Advantage on saves to avoid or end Charmed | `has_fey_ancestry` |
/// | Keen Senses | proficiency in Insight, Perception or Survival | `skills` |
/// | Trance | *"magic can't put you to sleep"* | `has_fey_ancestry`, again |
///
/// Two of those four arrive on one flag, and that is not a shortcut —
/// it is where the engine already put them. `FlagDrivenImmunity`'s Fey
/// Ancestry row suppresses **Charmed and Asleep** together, because the
/// only thing in this engine that installs `Asleep` is magic and RAW's
/// Trance clause is about magic. The elf is the species both halves of
/// that row were written for; the drow monster stat block has been
/// reading it since long before there was an elf to play.
///
/// **Keen Senses takes Perception**, of RAW's three. Insight and
/// Survival have no combat surface here — nothing in a fight rolls
/// either — and Perception is the check the engine asks constantly: the
/// Search action, every contest against a hidden creature, and the
/// passive score a Stealth roll is measured against.
///
/// **No Sunlight Sensitivity, on any of the three.** The
/// `creatures::drow` stat block carries it and this one does not, and
/// the difference is the book's: SRD 5.2 prints Sunlight Sensitivity on
/// the *monster* and prints nothing of the kind in the Elf species
/// entry. A playable drow fights at noon at full strength.
fn elf_chassis(
    name: &'static str,
    glyph: char,
    darkvision: u32,
    speed: f32,
) -> CreatureTemplate {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*SECOND_WIND);
    actions.push(&*ACTION_SURGE);
    CreatureTemplate {
        name,
        glyph,
        ac: 15,
        // 3d8+6 — a d8 chassis at the level every lineage build in this
        // family is written to, which puts the elf between the gnome's
        // 3d6 and the half-orc's 3d10. The species is a body, and this
        // one is neither the frailest nor the sturdiest on the bench.
        hitpoints: "3d8+6".parse().unwrap(),
        speed,
        strength: 11,
        dexterity: 16,
        constitution: 14,
        intelligence: 12,
        wisdom: 13,
        charisma: 12,
        // Keen Senses, plus the Stealth every one of these three builds
        // is shaped around — see each template.
        skills: HashSet::from([Skill::Perception, Skill::Stealth]),
        senses: HashSet::from([SpecialSense::Darkvision(darkvision)]),
        languages: HashSet::from([Language::Common, Language::Elvish]),
        cr: 2.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        // One slot at each of the two levels the Elven Lineages table
        // prints a spell at. RAW's wording is *"you can cast it once
        // without a spell slot, and you regain the ability to cast it in
        // that way when you finish a Long Rest"* — which, on a chassis
        // that has no other spellcasting to spend slots on, is exactly
        // what one slot per level per rest is. See each lineage for what
        // it has to spend them on; the High Elf's first slot is
        // deliberately empty.
        spell_slots_by_level: vec![1, 1],
        rolls_death_saves: true,
        proficient_saves: HashSet::from([
            AbilityScoreType::Dexterity,
            AbilityScoreType::Intelligence,
        ]),
        features: HashSet::from([SECOND_WIND_TAG, ACTION_SURGE_TAG]),
        // Fey Ancestry *and* Trance — see the docstring for why one flag
        // carries both.
        has_fey_ancestry: true,
        ..CreatureTemplate::defaults()
    }
}

/// **Drow** — *"The range of your Darkvision increases to 120 feet. You
/// also know the Dancing Lights cantrip."* Level 3: **Faerie Fire**.
/// Level 5: **Darkness**.
///
/// The lineage the engine's lighting layer was waiting for. Every other
/// creature on the roster that carries Darkness is a monster that came
/// with it; this is a *player* who can put out the lights, and who can
/// still see 120 feet after doing it. The two spells are the two halves
/// of one tactic and they are on one sheet on purpose:
///
///   - **Darkness** drives its tiles to `LightLevel::Dark`, which the
///     attack sweep reads through `sight_denied_between` — everybody
///     inside it swings blind in both directions, and the drow's
///     darkvision is what keeps them out of that trade.
///   - **Faerie Fire** is the answer to the same problem from the other
///     side: an outlined creature *"can't benefit from the Invisible
///     condition"* and hands out Advantage to every attacker, which is
///     what a drow does to whatever walks out of the dark at them.
///
/// Dancing Lights is the free light source a party with no torch does
/// not otherwise have, which matters more here than its cantrip tier
/// suggests: the loot table weights torches heavily precisely because a
/// dark board is unplayable without one.
///
/// Scimitar and shortbow, which is the Underdark kit the monster stat
/// block carries, at the DEX the chassis is built around — and without
/// the poison on its bolts, because that is the raider's training and
/// not the lineage's blood.
pub static DROW_ELF_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // RAW: *"The range of your Darkvision increases to 120 feet."*
    let mut t = elf_chassis("Drow Elf", 'E', 120, 30.);
    t.actions.push(&SCIMITAR);
    t.actions.push(&SHORTBOW);
    t.actions.push(&*DANCING_LIGHTS);
    t.actions.push(&*FAERIE_FIRE);
    t.actions.push(&*DARKNESS);
    t
});

/// **High Elf** — *"You know the Prestidigitation cantrip. Whenever you
/// finish a Long Rest, you can replace that cantrip with a different
/// cantrip from the Wizard spell list."* Level 3: **Detect Magic**.
/// Level 5: **Misty Step**.
///
/// The lineage whose table is two-thirds unplayable and one-third one of
/// the best spells in the game. Prestidigitation and Detect Magic are
/// both absent from the engine and neither is a gap worth filling:
/// nothing in a fight reads a chilled drink or a lit candle, and the
/// engine has no hidden-magic layer for a divination to reveal. What
/// ships is the row that matters, and it matters a great deal — Misty
/// Step is a bonus-action thirty-foot teleport that provokes no
/// opportunity attack, on a chassis that otherwise has no way out of a
/// melee it has lost.
///
/// So the first of the chassis's two slots has nothing to spend itself
/// on, and that is RAW rather than an oversight: the High Elf's level-3
/// row is a spell this engine does not carry, and inventing something
/// for it to hold would be inventing a lineage.
///
/// Longsword and longbow — the two weapons every edition has handed an
/// elf, and the pair that makes this the lineage that fights at both
/// ranges rather than choosing one.
pub static HIGH_ELF_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut t = elf_chassis("High Elf", 'e', 60, 30.);
    t.actions.push(&LONGSWORD);
    t.actions.push(&LONGBOW);
    t.actions.push(&*MISTY_STEP);
    t
});

/// **Wood Elf** — *"Your Speed increases to 35 feet. You also know the
/// Druidcraft cantrip."* Level 3: **Longstrider**. Level 5: **Pass
/// without Trace**.
///
/// Every benefit on this row is about not being where the enemy is, and
/// they compound in a way the table does not make obvious: 35 feet of
/// base Speed, plus Longstrider's ten, is a forty-five-foot step on a
/// chassis carrying a longbow. That is two tiles further than anything
/// else on the lineage bench can move and still shoot, which is the
/// whole of what a wood elf is for.
///
/// Pass without Trace is the other half — a Stealth bonus on a template
/// that is already proficient in it — and on a dark board the two
/// together are a creature the party never gets a turn against.
///
/// Druidcraft is not modeled and is the same kind of absence as the
/// High Elf's Prestidigitation: a cantrip whose entire text is weather
/// and flowers. The lineage loses nothing a fight would have noticed.
pub static WOOD_ELF_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // RAW: *"Your Speed increases to 35 feet."* The only other 35 on the
    // humanoid bench is the goliath's, and it is the wood elf's for a
    // very different reason — one closes, one never has to.
    let mut t = elf_chassis("Wood Elf", 'W', 60, 35.);
    t.actions.push(&LONGBOW);
    t.actions.push(&SHORTSWORD);
    t.actions.push(&*LONGSTRIDER);
    t.actions.push(&*PASS_WITHOUT_TRACE);
    t
});

/// SRD 5.2's three Elven Lineages, in the table's own order.
///
/// A cohort rather than three loose statics, for the reason the
/// goliath's ancestries and the dragon scale mails are ones: the family
/// has invariants a copy-paste breaks silently. All three must carry Fey
/// Ancestry — which is Trance as well, so an elf that lost the flag
/// would quietly become sleepable — and the two lineages RAW does *not*
/// give 120 feet of darkvision to must not have it.
pub fn elven_lineages() -> [&'static CreatureTemplate; 3] {
    [&DROW_ELF_TEMPLATE, &HIGH_ELF_TEMPLATE, &WOOD_ELF_TEMPLATE]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conditions::Condition;

    /// Every lineage is an elf, and only one of them is a drow.
    ///
    /// Three templates built by one function that takes the differences
    /// as arguments, which is the arrangement where a wrong argument is
    /// invisible: a high elf handed `120` reads exactly like a high elf,
    /// right up until it out-sees the drow it was supposed to be worse
    /// in the dark than.
    #[test]
    fn the_lineages_share_what_the_species_gives_and_differ_where_the_table_does() {
        for t in elven_lineages() {
            assert!(
                t.has_fey_ancestry,
                "{} lost Fey Ancestry, which is Trance as well — see the chassis",
                t.name
            );
            assert!(
                t.skills.contains(&Skill::Perception),
                "{} lost Keen Senses",
                t.name
            );
        }
        // RAW's table, read back off the three templates.
        let darkvision = |t: &CreatureTemplate| {
            t.senses
                .iter()
                .find_map(|s| match s {
                    SpecialSense::Darkvision(r) => Some(*r),
                    _ => None,
                })
                .expect("every elf has darkvision")
        };
        assert_eq!(darkvision(&DROW_ELF_TEMPLATE), 120);
        assert_eq!(darkvision(&HIGH_ELF_TEMPLATE), 60);
        assert_eq!(darkvision(&WOOD_ELF_TEMPLATE), 60);
        assert_eq!(
            WOOD_ELF_TEMPLATE.speed, 35.,
            "\"Your Speed increases to 35 feet\" is the wood elf's whole first row"
        );
        assert_eq!(DROW_ELF_TEMPLATE.speed, 30.);
        assert_eq!(HIGH_ELF_TEMPLATE.speed, 30.);
    }

    /// Trance and Fey Ancestry, proved at the install gate rather than
    /// at the flag.
    ///
    /// The flag is a fact about the sheet; this is the sentence RAW
    /// writes. A species trait that is true in the struct literal and
    /// does nothing at `add_condition` is the failure the whole
    /// `FlagDrivenImmunity` cohort exists to make impossible, and it is
    /// worth one test per species that leans on it.
    #[test]
    fn an_elf_cannot_be_charmed_or_magically_slept() {
        use crate::actors::actor_template::ActorInstance;
        use crate::conditions::ConditionTimer;
        use crate::engine::dice::FastRandRoller;
        use crate::engine::types::Coordinate;

        for t in elven_lineages() {
            let mut a = ActorInstance::from_creature_template(
                t,
                Coordinate::new(0, 0),
                1,
                &mut FastRandRoller::with_seed(0),
                0,
            )
            .expect("every lineage instantiates");
            a.add_condition(Condition::Charmed, ConditionTimer::Rounds(5));
            a.add_condition(Condition::Asleep, ConditionTimer::Rounds(5));
            assert!(
                !a.has_condition(Condition::Charmed),
                "{}: Fey Ancestry",
                t.name
            );
            assert!(!a.has_condition(Condition::Asleep), "{}: Trance", t.name);
        }
    }

    /// The lineage table's spells are on the lineage's sheet.
    ///
    /// Three templates, three disjoint spell lists, one builder: the
    /// arrangement where a row pushed onto the wrong template compiles
    /// and reads correctly and is simply a different species.
    #[test]
    fn each_lineage_carries_its_own_rows_of_the_table() {
        use crate::actors::actor_template::ActorInstance;
        use crate::engine::dice::FastRandRoller;
        use crate::engine::types::Coordinate;

        let sheet = |t: &'static CreatureTemplate| {
            ActorInstance::from_creature_template(
                t,
                Coordinate::new(0, 0),
                1,
                &mut FastRandRoller::with_seed(0),
                0,
            )
            .expect("every lineage instantiates")
        };
        let drow = sheet(&DROW_ELF_TEMPLATE);
        for row in ["dancing lights", "faerie fire", "darkness"] {
            assert!(drow.find_action(row).is_some(), "the drow's {row}");
        }
        assert!(
            drow.find_action("misty step").is_none(),
            "Misty Step is the High Elf's row, not the drow's"
        );

        let high = sheet(&HIGH_ELF_TEMPLATE);
        assert!(high.find_action("misty step").is_some());
        assert!(
            high.find_action("darkness").is_none(),
            "Darkness is the drow's row"
        );

        let wood = sheet(&WOOD_ELF_TEMPLATE);
        for row in ["longstrider", "pass without trace"] {
            assert!(wood.find_action(row).is_some(), "the wood elf's {row}");
        }
        assert!(
            wood.find_action("faerie fire").is_none(),
            "Faerie Fire is the drow's row"
        );
    }
}
