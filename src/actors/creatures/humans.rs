use crate::actions::class_features::{ACTION_SURGE, ACTION_SURGE_TAG, SECOND_WIND, SECOND_WIND_TAG};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::feats::ALERT_TAG;
use crate::actions::monster_attacks::{LONGBOW, LONGSWORD};
use crate::actions::species::RESOURCEFUL_TAG;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size, Skill};
use std::collections::HashSet;
use std::sync::LazyLock;

/// **Human** — SRD 5.2's plainest species, and the last one on the
/// book's list the engine had no sheet for.
///
/// > *Creature Type: Humanoid. Size: Medium or Small, chosen when you
/// > select this species. Speed: 30 feet.*
///
/// Three traits, none of them a resistance, a sense or an immunity:
///
///   - **Resourceful.** *"You gain Heroic Inspiration whenever you
///     finish a Long Rest."* The only one with machinery behind it —
///     one charge on `SELF_DISADVANTAGE_CANCELLERS`, spent to straighten
///     a disadvantaged d20. See `species::RESOURCEFUL_TAG` for how far
///     that is from RAW's "reroll any die" and why the distance is in
///     the safe direction.
///   - **Skillful.** *"You gain proficiency in one skill of your
///     choice."* Taken here as **Perception**, which is the skill this
///     engine rolls most: the Search action, every contest against
///     something hidden, and the passive score a Stealth check is
///     measured against.
///   - **Versatile.** *"You gain an Origin feat of your choice (Skilled
///     is recommended)."* Taken here as **Alert**, because Skilled is a
///     third and fourth skill proficiency and the engine has nothing to
///     roll them for. Alert is initiative, which decides who acts
///     first in every fight this engine runs, and it is the Origin feat
///     with the largest combat surface on the list.
///
/// **A human is a species made of choices**, and this is one human, not
/// the species. That is the same compromise the fifteen dragonborn and
/// the six goliaths make from the other direction: where RAW prints a
/// table the engine prints a template per row, and where RAW prints
/// *"of your choice"* over an open list the engine picks once and says
/// which. The two picks above are written down here rather than rolled,
/// so a reader can disagree with them by name.
///
/// **Medium, of RAW's two.** The Small option exists and buys nothing
/// good in a grid engine — a Small creature occupies the same tile a
/// Medium one does and is easier to grapple.
///
/// The chassis is a fighter's, and deliberately the most ordinary one on
/// the lineage bench: longsword, longbow, Second Wind, Action Surge, and
/// no third thing. Every other build in this family leans on something
/// its species gave it. The human's advantage is that it has no
/// weakness to work around, which is only visible beside the others.
pub static HUMAN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&LONGSWORD);
    actions.push(&LONGBOW);
    actions.push(&*SECOND_WIND);
    actions.push(&*ACTION_SURGE);
    CreatureTemplate {
        name: "Human",
        // 'U' — 'H' is the Half-Orc Marauder's and 'h' is the Halfling
        // Scout's, and the two most obvious letters for this species are
        // both already carrying one of its neighbours on the bench.
        glyph: 'U',
        ac: 16,
        // 3d10+9, the half-orc's pool exactly. The two are the family's
        // two plain fighters and they should read as a lateral choice:
        // what the half-orc has in Relentless Endurance and Savage
        // Attacks, the human has in a feat and a reroll.
        hitpoints: "3d10+9".parse().unwrap(),
        strength: 16,
        dexterity: 14,
        constitution: 16,
        intelligence: 10,
        wisdom: 12,
        charisma: 11,
        // Skillful, spent. See the docstring.
        skills: HashSet::from([Skill::Perception]),
        // No Darkvision, which is the species' one real cost: the human
        // is the only build on this bench that needs the torch the loot
        // table keeps handing out.
        languages: HashSet::from([Language::Common]),
        cr: 2.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        rolls_death_saves: true,
        proficient_saves: HashSet::from([
            AbilityScoreType::Strength,
            AbilityScoreType::Constitution,
        ]),
        // Resourceful (the species), Alert (Versatile, spent), and the
        // fighter chassis underneath both.
        features: HashSet::from([
            RESOURCEFUL_TAG,
            ALERT_TAG,
            SECOND_WIND_TAG,
            ACTION_SURGE_TAG,
        ]),
        ..CreatureTemplate::defaults()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::Coordinate;

    fn sheet() -> ActorInstance {
        ActorInstance::from_creature_template(
            &HUMAN_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .expect("the human instantiates")
    }

    /// All three of RAW's traits are on the sheet, and the one with a
    /// pool has a pool.
    ///
    /// Resourceful is the trait worth checking twice, because its depth
    /// is a *default* rather than a number anybody wrote down: a tag
    /// with no row in `FEATURE_CHARGES` gets one charge per rest, which
    /// is what RAW hands a Human and so what this trait needs. Nothing
    /// in the table says so — the assertion below is where that reading
    /// is written down.
    #[test]
    fn the_human_carries_its_three_traits_and_can_spend_the_one_that_costs() {
        let a = sheet();
        assert!(
            a.has_passive_feature(RESOURCEFUL_TAG),
            "Resourceful is the species' only charged trait"
        );
        assert!(
            a.feature_available(RESOURCEFUL_TAG),
            "a Human wakes up holding one Heroic Inspiration, which is \
             the charge an untabled tag gets by default"
        );
        assert!(
            a.has_passive_feature(ALERT_TAG),
            "Versatile, spent on an Origin feat"
        );
        assert!(
            a.has_skill(Skill::Perception),
            "Skillful, spent on the skill the engine actually rolls"
        );
    }

    /// The Heroic Inspiration is one, and it is spent.
    ///
    /// The whole of the trait's combat surface: a disadvantaged d20
    /// comes back Normal once, and the second one does not.
    #[test]
    fn resourceful_straightens_one_roll_a_rest_and_no_more() {
        use crate::engine::dice::RollMode;
        use crate::engine::terrain_gen::TerrainGenParams;

        let mut e = crate::engine::encounter::EncounterInstance::empty(
            &TerrainGenParams {
                width: 12,
                height: 12,
                branch_depth: 0,
                branch_prob: 0.0,
            },
            Some(0),
        );
        let human = e
            .instantiate_creature(&HUMAN_TEMPLATE, Coordinate::new(2, 2), 0, 0)
            .expect("the human fits");
        assert_eq!(
            e.cancel_disadvantage_with_luck(human, RollMode::Disadvantage),
            RollMode::Normal,
            "the token is there to be spent"
        );
        assert_eq!(
            e.cancel_disadvantage_with_luck(human, RollMode::Disadvantage),
            RollMode::Disadvantage,
            "RAW: \"you can't have more than one at a time\""
        );
    }
}
