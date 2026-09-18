//! The **Otherworldly Steed** — the body SRD 5.2's *Find Steed* puts
//! under its caster, in the three shapes RAW lets them choose between.
//!
//! Three stat blocks in one file for the reason
//! [`crate::actors::creatures::summoned_spirits`] keeps eight in one:
//! none of them is a creature the world contains. Nothing generates an
//! Otherworldly Steed, nothing rolls one into an encounter, and no
//! bestiary template carries one in its action list. Each exists because
//! one spell names it, and the three differ by exactly the two lines RAW
//! says they differ by — so splitting them across three files would
//! scatter one decision into three places where nothing could compare
//! them.
//!
//! ## What the book prints
//!
//! > **Otherworldly Steed.** Large Celestial, Fey, or Fiend (Your
//! > Choice), Neutral. AC 10 + 1 per spell level. HP 5 + 10 per spell
//! > level (the steed has a number of Hit Dice \[d10s\] equal to the
//! > spell's level). Speed 60 ft., Fly 60 ft. (requires level 4+ spell).
//! > Str 18, Dex 12, Con 14, Int 6, Wis 12, Cha 8.
//!
//! Almost every line of that is an expression in the spell's level
//! rather than a number, which is why this file is written *at the
//! spell's printed level of 2* and the rest rides
//! [`crate::actions::spells::SummonScaling`]:
//!
//! | line | at level 2 | how it grows |
//! |------|-----------|--------------|
//! | AC | 12 | `ac_per_level: 1` |
//! | HP | 25 (`2d10+14`) | `hp_per_level: 10` |
//! | Fly | none | `fly_from_level: Some((4, 60))` |
//! | Slam | `1d8 + 2` | fixed — see below |
//!
//! The hit dice are RAW's own — *"a number of Hit Dice \[d10s\] equal to
//! the spell's level"* is two d10s at level 2 — and the flat `+14` is
//! what brings their average to the `5 + 10 × 2` the same line prints.
//! Two numbers for one clause, because RAW gives two and they do not
//! agree: a creature with 2d10 and Constitution 14 would average 15.
//! The printed total wins and the dice stay the book's.
//!
//! ## The three branches
//!
//! RAW's option tables are usually collapsed to one branch in this
//! engine — see `SummonSpell::template` — and this one is not, because
//! here the table is the spell. The three branches differ in:
//!
//! | branch | slam damage | bonus action |
//! |--------|-------------|--------------|
//! | Celestial | Radiant | **Healing Touch** — `2d8 + 2` to a creature within 5 ft |
//! | Fey | Psychic | **Fey Step** — teleport, with its rider, up to 60 ft |
//! | Fiend | Necrotic | **Fell Glare** — WIS save or Frightened, 60 ft |
//!
//! A damage type and a whole extra action are not a flavour column, and
//! a paladin choosing between them is making the same kind of choice as
//! a druid choosing between Summon Fey and Summon Undead. So: three
//! templates, and three `SummonSpell` declarations pointing at them.
//!
//! Each branch ability is printed *"Recharges after a Long Rest"*, which
//! inside one encounter means once — see
//! [`crate::actors::actor_template::NEVER_RECHARGES`], the constant that
//! clause needed and did not have.
//!
//! ## What is here and what is not
//!
//! **Life Bond** — *"When you regain Hit Points from a level 1+ spell,
//! the steed regains the same number of Hit Points if you're within 5
//! feet of it"* — is not on the template at all, because it is not a
//! property of the body: it names a specific other creature. It rides
//! [`crate::conditions::Condition::LifeBonded`], installed by the spell
//! and back-linked to the caster, and resolves at
//! `EncounterInstance::mirror_heals_onto_life_bonds`.
//!
//! **Telepathy 1 mile (works only with you)** has no surface on a board
//! sixty feet across and is dropped rather than approximated. The steed
//! ships with no languages, which is the honest reading: it speaks none.
//!
//! **The saves** are the modifiers. RAW's stat block prints MOD and SAVE
//! as the same number in all six columns, so the steed is proficient in
//! nothing and `proficient_saves` stays empty.

use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::action_template::{
    Action, TargetingSchema, bonus_action_only, first_target_location,
};
use crate::actions::monster_attacks::{
    RechargingAllyHeal, RechargingAttack, SaveOrCondition, SimpleWeapon,
};
use crate::actors::actor_template::{CreatureTemplate, NEVER_RECHARGES};
use crate::conditions::{Condition, ConditionTimer};
use crate::engine::action_overrides::ActionOverride;
use crate::engine::dice::Dice;
use crate::engine::encounter::EncounterInstance;
use crate::engine::side_effects::{ApplicableSideEffect, Resource};
use crate::engine::types::{
    AbilityScoreType, Coordinate, CreatureType, DamageType, Size,
};
use std::collections::HashSet;
use std::sync::LazyLock;

/// The level Find Steed is written at, and the level every number in
/// this file is written at with it.
pub const FIND_STEED_BASE_LEVEL: u32 = 2;

/// The steed's walking speed, in feet — RAW's `Speed 60 ft.`, the
/// warhorse's exactly.
pub const STEED_SPEED: f32 = 60.;

/// The flying speed RAW switches on at a level-4 slot, in feet, and the
/// slot it switches on at. Declared here beside the walking speed rather
/// than at the three spell declarations, so the number and the threshold
/// live with the stat block that prints them.
pub const STEED_FLY_FROM_LEVEL: u32 = 4;
pub const STEED_FLY_SPEED: u32 = 60;

/// Sixty feet, in tiles — the reach of both the Fey branch's teleport
/// and the Fiend branch's glare, which RAW gives the same number.
const SIXTY_FEET: isize = 24;

/// The DC the Fiend branch's glare asks for.
///
/// RAW is *"DC equals your spell save DC"*, and a `&'static` stat block
/// has no channel back to whoever conjured it — the same departure, for
/// the same reason, that `summoned_spirits`' `SPIRIT_SAVE_DC` makes for
/// the whole Tasha's family. The number is that constant's, and
/// deliberately: a paladin only has a level-2 slot from character level
/// 5, where proficiency is +3 and the Charisma that bought the oath is
/// +3, so `8 + 3 + 3` is what the caster this steed belongs to would
/// have handed it.
const STEED_SAVE_DC: i32 = 14;

/// The flat term the three branches' attack lines carry: RAW's *"1d8
/// plus the spell's level"*, at the spell's printed level.
///
/// Fixed rather than scaled, which `SimpleWeapon::flat_damage_bonus`
/// explains at length and which is the same ceiling every conjured
/// stat block in the engine sits under: an attack is a `&'static`
/// shared by every copy of the creature, so there is nowhere for a
/// per-body damage term to live.
const SLAM_FLAT_BONUS: i32 = FIND_STEED_BASE_LEVEL as i32;

/// **Otherworldly Slam**, Celestial — *"1d8 plus the spell's level of
/// Radiant"*.
///
/// `flat_melee` rather than `melee`, which is the whole point: RAW's Hit
/// line prints no ability modifier at all, and a steed swinging with
/// Strength 18 behind it would land four points the book does not
/// print — on a level-2 spell, most of a second hit.
pub static CELESTIAL_SLAM: SimpleWeapon = SimpleWeapon::flat_melee(
    "otherworldly slam",
    &["slam", "os"],
    AbilityScoreType::Strength,
    Dice::new(1, 8),
    DamageType::Radiant,
)
.plus_flat(SLAM_FLAT_BONUS);

/// **Otherworldly Slam**, Fey — the same line with Psychic in it.
pub static FEY_SLAM: SimpleWeapon = SimpleWeapon::flat_melee(
    "otherworldly slam",
    &["slam", "os"],
    AbilityScoreType::Strength,
    Dice::new(1, 8),
    DamageType::Psychic,
)
.plus_flat(SLAM_FLAT_BONUS);

/// **Otherworldly Slam**, Fiend — the same line with Necrotic in it.
pub static FIEND_SLAM: SimpleWeapon = SimpleWeapon::flat_melee(
    "otherworldly slam",
    &["slam", "os"],
    AbilityScoreType::Strength,
    Dice::new(1, 8),
    DamageType::Necrotic,
)
.plus_flat(SLAM_FLAT_BONUS);

/// **Healing Touch** (Celestial Only; Recharges after a Long Rest) —
/// *"One creature within 5 feet of the steed regains a number of Hit
/// Points equal to 2d8 plus the spell's level."*
///
/// The third row on the [`RechargingAllyHeal`] chassis, beside the
/// unicorn's and the deva's, and the only one of the three whose
/// recharge clause the engine can state exactly rather than approximate
/// — see [`NEVER_RECHARGES`].
pub static STEED_HEALING_TOUCH: RechargingAllyHeal = RechargingAllyHeal::touch(
    "steed healing touch",
    &["sht", "steed-touch"],
    Dice::new(2, 8),
    STEED_HEALING_TOUCH_KEY,
)
.plus_flat(FIND_STEED_BASE_LEVEL as i32)
.as_bonus_action();

/// The recharge pool keys for the three branch abilities.
///
/// One key each rather than a shared one, because the three never
/// co-occur: a steed is one branch, and a pool shared between abilities
/// nothing can hold at once is a pool with no second reader. Named
/// constants rather than string literals because each is spelled twice
/// — once on the action, once on its template's `recharge_abilities` —
/// and a mismatch fails silently as "the steed never uses it".
pub const STEED_HEALING_TOUCH_KEY: &str = "steed_healing_touch";
pub const FEY_STEP_KEY: &str = "fey_step";
pub const FELL_GLARE_KEY: &str = "fell_glare";

/// **Fell Glare** (Fiend Only; Recharges after a Long Rest) — *"Wisdom
/// Saving Throw: DC equals your spell save DC, one creature within 60
/// feet the steed can see. Failure: The target has the Frightened
/// condition until the end of your next turn."*
///
/// The sixth row on the [`SaveOrCondition`] chassis and the first that
/// installs something other than a charm — see that type for why the
/// condition became a field.
///
/// `UntilStartOfNextTurn` for RAW's *"until the end of your next turn"*,
/// which is half a round short and deliberately so: it is the same
/// trade `summoned_spirits`' Fey Blade makes, and for the same reason.
/// `Rounds(1)` lapses at the round-end sweep, which runs *after* the
/// victim's turn, so a fright installed on the steed's turn would cost
/// its target two turns rather than one.
static FELL_GLARE_SAVE: SaveOrCondition = SaveOrCondition::action(
    "fell glare",
    &["glare", "fg"],
    SIXTY_FEET,
    STEED_SAVE_DC,
    ConditionTimer::UntilStartOfNextTurn,
    "fell glare",
)
.installing(Condition::Frightened)
.as_bonus_action();

/// Fell Glare with its once-a-day gate on it.
pub static FELL_GLARE: LazyLock<RechargingAttack> = LazyLock::new(|| RechargingAttack {
    display_name: "fell glare",
    sub_attack: &FELL_GLARE_SAVE,
    recharge_key: FELL_GLARE_KEY,
});

/// **Fey Step** (Fey Only; Recharges after a Long Rest) — *"The steed
/// teleports, along with its rider, to an unoccupied space of your
/// choice up to 60 feet away from itself."*
///
/// Its own `impl Action` rather than a row on a chassis, because the
/// engine has no teleport chassis to be a row on: Misty Step, Far Step,
/// Dimension Door and Hidden Paths are each a hand-written impl around
/// one [`crate::engine::side_effects::TeleportActor`]. A fifth is worth
/// less than a sixth would be, and this one differs from all four in the
/// clause that matters.
///
/// **"Along with its rider" is free**, and that is the interesting part.
/// `EncounterInstance::relocate_actor` already carries a mount's
/// passenger — a rider's footprint *is* the mount's — so a teleport
/// aimed at the steed takes the paladin with it without this action
/// knowing that mounts exist. The same routine does the opposite for a
/// teleport aimed at the *rider*, dismounting them first, which is also
/// RAW: Misty Step moves you, not the horse you were sitting on.
pub struct FeyStep {}

impl Action for FeyStep {
    fn name(&self) -> &str {
        "fey step"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["fs", "step"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SinglePoint
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(SIXTY_FEET)
    }
    fn requires_los(&self) -> bool {
        // RAW: "a space of your choice", with no "you can see" on it —
        // but the engine's point-targeting lane is line-of-sight
        // throughout and a blind 60-foot jump through a wall is not what
        // the sentence is reaching for.
        true
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn damage_types(&self) -> Vec<DamageType> {
        Vec::new()
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_only()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // RAW's "unoccupied space": the whole Large footprint has to
        // fit, which is what `can_move_to` asks and what a 2×2 body
        // makes a real question rather than a formality.
        let Some(point) = first_target_location(target_locations) else {
            return false;
        };
        encounter.can_move_to(caster_id, point)
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        vec![Box::new(crate::engine::side_effects::TeleportActor {
            actor_id: caster_id,
            dest: point,
        })]
    }
}

static FEY_STEP_MOVE: LazyLock<FeyStep> = LazyLock::new(|| FeyStep {});

/// Fey Step with its once-a-day gate on it.
pub static FEY_STEP: LazyLock<RechargingAttack> = LazyLock::new(|| RechargingAttack {
    display_name: "fey step",
    sub_attack: &*FEY_STEP_MOVE,
    recharge_key: FEY_STEP_KEY,
});

/// Everything the three branches share, so the three literals below
/// differ by exactly the lines RAW says they differ by.
///
/// A function rather than a `LazyLock` the three clone off, because
/// `actions` is the field that varies and a clone would have to throw
/// the shared one away — see `phantom_steeds`, where the clone tail is
/// load-bearing for the opposite reason.
fn steed_chassis(
    name: &'static str,
    creature_type: CreatureType,
    actions: Vec<&'static (dyn Action + Send + Sync)>,
    recharge_key: &'static str,
) -> CreatureTemplate {
    CreatureTemplate {
        name,
        // 'H', with the rest of the rideable cohort — the riding horse,
        // the warhorse, the phantom steed. A player who sees an 'H'
        // beside a paladin should think "somebody is about to get on
        // that", and which steed it is matters less than that it is one.
        glyph: 'H',
        // RAW's "AC 10 + 1 per spell level" at the spell's own level.
        ac: 10 + FIND_STEED_BASE_LEVEL,
        // RAW's hit dice with RAW's total — see the module docs.
        hitpoints: "2d10+14".parse().unwrap(),
        speed: STEED_SPEED,
        strength: 18,
        dexterity: 12,
        constitution: 14,
        intelligence: 6,
        wisdom: 12,
        charisma: 8,
        // RAW prints MOD and SAVE alike in all six columns, and no
        // senses beyond Passive Perception 11.
        senses: HashSet::new(),
        // "Telepathy 1 mile (works only with you)" is not a language and
        // has no board surface — see the module docs.
        languages: HashSet::new(),
        // "CR None (XP 0; PB equals your Proficiency Bonus)". The engine
        // needs a number; 1 is the rung a level-2 slot buys, and the one
        // `summoned_spirits`' Bestial Spirit — the other level-2 summon
        // in the engine — already sits on.
        cr: 1.0,
        size: Size::Large,
        creature_type,
        actions,
        // The whole reason the spell exists. See `engine::mounts`.
        mountable: true,
        recharge_abilities: vec![(recharge_key, NEVER_RECHARGES)],
        ..CreatureTemplate::defaults()
    }
}

/// **Celestial** Otherworldly Steed — radiant slam, and a bonus-action
/// heal for whoever is standing next to it.
///
/// The branch a paladin takes when the party's problem is staying
/// upright: `2d8 + 2` once a fight, on a body that also carries them
/// sixty feet a turn. It is the only branch whose bonus action does
/// nothing to the enemy, and on a half-caster whose own healing is a
/// finite pool of Lay on Hands points, that is worth more than it looks.
pub static CELESTIAL_STEED_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&CELESTIAL_SLAM);
    actions.push(&STEED_HEALING_TOUCH);
    steed_chassis(
        "Celestial Steed",
        CreatureType::Celestial,
        actions,
        STEED_HEALING_TOUCH_KEY,
    )
});

/// **Fey** Otherworldly Steed — psychic slam, and a sixty-foot blink
/// that takes its rider along.
///
/// The branch that is about the board rather than about damage. Sixty
/// feet is further than any speed in the engine bar the phantom steed's
/// hundred, it costs a bonus action rather than movement, and it crosses
/// a chasm, a wall of fire and an enemy line alike. A paladin in the
/// saddle arrives in the enemy's back rank with their Action untouched.
pub static FEY_STEED_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&FEY_SLAM);
    actions.push(&*FEY_STEP);
    steed_chassis("Fey Steed", CreatureType::Fey, actions, FEY_STEP_KEY)
});

/// **Fiend** Otherworldly Steed — necrotic slam, and a glare that puts
/// one creature on the back foot.
///
/// The branch that fights. Frightened is the condition that costs an
/// enemy caster their advance and an enemy brute its advantage, and
/// sixty feet of range means the steed can land it on something it has
/// no intention of reaching this turn.
pub static FIEND_STEED_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&FIEND_SLAM);
    actions.push(&*FELL_GLARE);
    steed_chassis("Fiend Steed", CreatureType::Fiend, actions, FELL_GLARE_KEY)
});

/// Every branch, for the sweeps that want to say something about all
/// three at once.
///
/// A function rather than a `pub static … : &[&LazyLock<CreatureTemplate>]`,
/// which is what it was first written as and which the roster's own
/// reachability sweep reads as a fourth template declaration — see
/// `actors::creatures::reachability`. The sweep scans source text for
/// `pub static NAME: … LazyLock<CreatureTemplate>`, so an aggregator
/// spelled that way is a template nothing can put on a board, and the
/// only way to satisfy it would have been to lie about the aggregator
/// somewhere. It is also the shape the sweep already follows for
/// families — `swarms::all_swarm_templates()` — so this reads like the
/// rest of the roster.
pub fn all_steed_templates() -> [&'static LazyLock<CreatureTemplate>; 3] {
    [
        &CELESTIAL_STEED_TEMPLATE,
        &FEY_STEED_TEMPLATE,
        &FIEND_STEED_TEMPLATE,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::action_template::ActionExecutionInfo;
    use crate::actors::actor_template::ActorInstance;
    use crate::engine::actor_gen::ActorGenParams;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::terrain_gen::TerrainGenParams;

    /// A featureless 20×20 room with nobody in it, for the two tests
    /// that need a board rather than a stat block.
    fn empty_encounter() -> EncounterInstance {
        let tp = TerrainGenParams {
            width: 20,
            height: 20,
            branch_depth: 0,
            branch_prob: 0.0,
        };
        let ap = ActorGenParams {
            cr_target: 0.0,
            n_teams: 0,
            pc_template: None,
            start_team: 0,
        };
        EncounterInstance::from_params(&tp, &ap, Some(0)).unwrap()
    }

    fn make(template: &'static LazyLock<CreatureTemplate>) -> ActorInstance {
        ActorInstance::from_creature_template(
            template,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    /// The header lines RAW writes as expressions, evaluated at the
    /// spell's printed level. Every one of them is a number a higher
    /// slot moves, and the arithmetic that moves it lives one file over
    /// in `SummonScaling` — so what this pins is the *floor*.
    #[test]
    fn every_branch_is_the_books_stat_block_at_level_two() {
        for template in all_steed_templates() {
            let steed = make(template);
            let name = steed.name().to_string();
            // AC 10 + 1 per spell level.
            assert_eq!(steed.armor_class(), 12, "{name}");
            // HP 5 + 10 per spell level, rolled off the book's own hit
            // dice — so what a given steed has varies inside 2d10+14
            // and what the *expression* averages does not.
            assert!(
                (16..=34).contains(&steed.max_hitpoints()),
                "{name}: {} is outside 2d10+14",
                steed.max_hitpoints()
            );
            assert_eq!(
                template.hitpoints.average_roll(),
                25.0,
                "{name}: the book prints 5 + 10 per spell level"
            );
            assert_eq!(steed.speed(), STEED_SPEED, "{name}");
            assert!(
                !steed.has_innate_flight(),
                "{name}: flight is a level-4 slot"
            );
            assert_eq!(steed.size(), Size::Large, "{name}");
            // Str 18, Dex 12, Con 14, Int 6, Wis 12, Cha 8 — the
            // book's six columns, read as the modifiers it prints
            // beside them.
            for (ability, modifier) in [
                (AbilityScoreType::Strength, 4),
                (AbilityScoreType::Dexterity, 1),
                (AbilityScoreType::Constitution, 2),
                (AbilityScoreType::Intelligence, -2),
                (AbilityScoreType::Wisdom, 1),
                (AbilityScoreType::Charisma, -1),
            ] {
                assert_eq!(
                    steed.ability_modifier(ability),
                    modifier,
                    "{name}: {ability:?}"
                );
            }
            assert!(
                template.mountable,
                "{name}: a steed nobody can ride is not a steed"
            );
            assert!(
                steed.find_action("otherworldly slam").is_some(),
                "{name} has no slam"
            );
        }
    }

    /// The two lines the three branches are *for*. A refactor that gave
    /// them all one damage type, or dropped a branch ability onto the
    /// wrong chassis, would leave three identical horses.
    #[test]
    fn each_branch_carries_its_own_damage_type_and_its_own_bonus_action() {
        let rows: [(&'static LazyLock<CreatureTemplate>, DamageType, &str, &str); 3] = [
            (
                &CELESTIAL_STEED_TEMPLATE,
                DamageType::Radiant,
                "steed healing touch",
                STEED_HEALING_TOUCH_KEY,
            ),
            (&FEY_STEED_TEMPLATE, DamageType::Psychic, "fey step", FEY_STEP_KEY),
            (
                &FIEND_STEED_TEMPLATE,
                DamageType::Necrotic,
                "fell glare",
                FELL_GLARE_KEY,
            ),
        ];
        let mut seen_types = HashSet::new();
        for (template, damage_type, ability, key) in rows {
            let steed = make(template);
            let name = steed.name().to_string();
            let slam = steed.find_action("otherworldly slam").expect("a slam");
            assert_eq!(slam.damage_types(), vec![damage_type], "{name}");
            assert!(
                steed.find_action(ability).is_some(),
                "{name} is missing {ability}"
            );
            // And carries nobody else's.
            for (_, _, other, _) in rows {
                assert!(
                    other == ability || steed.find_action(other).is_none(),
                    "{name} should not have {other}"
                );
            }
            assert!(
                steed.is_recharge_available(key),
                "{name}'s {ability} starts the fight up"
            );
            seen_types.insert(damage_type);
        }
        assert_eq!(seen_types.len(), 3, "three branches, three damage types");
    }

    /// RAW's Hit line is *"1d8 plus the spell's level"* with no ability
    /// modifier in it, and the steed's Strength is +4 — so a slam built
    /// on the ordinary constructor would land six points where the book
    /// prints three. The `flat_melee` + `plus_flat` pair is what says
    /// so, and this is what keeps it said.
    #[test]
    fn the_slam_carries_the_spells_level_and_not_the_steeds_strength() {
        for slam in [&CELESTIAL_SLAM, &FEY_SLAM, &FIEND_SLAM] {
            assert_eq!(slam.damage_dice, Dice::new(1, 8));
            assert!(
                slam.damage_ability.is_none(),
                "the book's Hit line prints no modifier"
            );
            assert_eq!(slam.flat_damage_bonus, FIND_STEED_BASE_LEVEL as i32);
        }
    }

    /// *"Recharges after a Long Rest"* is not *"Recharge 5–6"*, and the
    /// difference is the whole reason `NEVER_RECHARGES` exists. A
    /// threshold a d6 could reach would hand the steed a second glare
    /// in the same fight.
    #[test]
    fn the_branch_abilities_do_not_come_back_inside_a_fight() {
        for template in all_steed_templates() {
            assert_eq!(
                template.recharge_abilities.len(),
                1,
                "{}: one branch, one pool",
                template.name
            );
            for (key, min_roll) in &template.recharge_abilities {
                assert_eq!(
                    *min_roll, NEVER_RECHARGES,
                    "{}: {key} recharges on a long rest and nothing else",
                    template.name
                );
                assert!(*min_roll > 6, "no face of a d6 reaches it");
            }
        }
    }

    /// The branch abilities are printed under **Bonus Actions**, and on
    /// a mount that is the difference between a steed that glares and a
    /// steed that glares *and* carries its rider into the fight.
    #[test]
    fn every_branch_ability_is_a_bonus_action() {
        assert!(STEED_HEALING_TOUCH.bonus_action);
        assert!(FELL_GLARE_SAVE.bonus_action);
        let empty = empty_encounter();
        assert_eq!(
            FEY_STEP_MOVE.cost(&empty, 0, None, None, None),
            bonus_action_only()
        );
    }

    /// Fell Glare installs the condition its own sentence names. The
    /// chassis under it spent its life installing `Charmed` and nothing
    /// else; this is the row that made the field.
    #[test]
    fn the_glare_frightens_rather_than_charms() {
        assert_eq!(FELL_GLARE_SAVE.condition, Condition::Frightened);
        assert_eq!(FELL_GLARE_SAVE.save_ability, AbilityScoreType::Wisdom);
        assert_eq!(FELL_GLARE_SAVE.reach, SIXTY_FEET);
        assert_eq!(FELL_GLARE_SAVE.installs_condition(), Some(Condition::Frightened));
    }

    /// Unused-but-declared is the failure mode that makes a summon
    /// silently never use its own trait: the action's recharge key and
    /// the template's row are matched by string, and a mismatch fails
    /// closed.
    #[test]
    fn every_branch_abilitys_key_is_one_its_template_declares() {
        let rows: [(&'static LazyLock<CreatureTemplate>, &str); 3] = [
            (&CELESTIAL_STEED_TEMPLATE, STEED_HEALING_TOUCH_KEY),
            (&FEY_STEED_TEMPLATE, FEY_STEP_KEY),
            (&FIEND_STEED_TEMPLATE, FELL_GLARE_KEY),
        ];
        for (template, key) in rows {
            assert!(
                template.recharge_abilities.iter().any(|(k, _)| *k == key),
                "{} declares no pool called {key}",
                template.name
            );
        }
        assert_eq!(STEED_HEALING_TOUCH.recharge_key, STEED_HEALING_TOUCH_KEY);
        assert_eq!(FELL_GLARE.recharge_key, FELL_GLARE_KEY);
        assert_eq!(FEY_STEP.recharge_key, FEY_STEP_KEY);
    }

    /// A Fey Step onto a tile the steed's own 2×2 body does not fit on
    /// is not "an unoccupied space", and the validator is what says so —
    /// on the steed, which is the only creature on the roster for which
    /// the distinction is routine.
    #[test]
    fn the_fey_step_refuses_a_space_the_steed_does_not_fit_in() {
        let mut e = empty_encounter();
        let steed = e
            .instantiate_creature(&FEY_STEED_TEMPLATE, Coordinate::new(2, 2), 0, 0)
            .expect("a steed");
        let far = Coordinate::new(60, 60);
        assert!(
            !ActionExecutionInfo::new(&*FEY_STEP, steed, None, Some(vec![far]), None)
                .validate(&e),
            "sixty tiles is past sixty feet"
        );
        let near = Coordinate::new(6, 6);
        assert!(
            ActionExecutionInfo::new(&*FEY_STEP, steed, None, Some(vec![near]), None)
                .validate(&e),
            "an empty patch of floor four tiles away is a legal landing"
        );
    }
}
