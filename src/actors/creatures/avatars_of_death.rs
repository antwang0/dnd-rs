//! **Avatar of Death** — the last stat block in SRD 5.2 the bestiary
//! did not carry, and the only one that is not really a creature.
//!
//! Everything about it is an expression in somebody else's sheet. Its
//! hit points are *"Half the HP maximum of its summoner"*. Its
//! proficiency bonus *"equals its summoner's"*. The number of times it
//! swings is *"half the summoner's Proficiency Bonus (rounded up)"*.
//! Its languages are *"All languages known to its summoner"*. It has no
//! challenge rating and is worth no experience, because it is not an
//! encounter — it is a consequence, and the thing it is a consequence
//! of is turning over the Skull card of a Deck of Many Things.
//!
//! That shape is why three small pieces of engine had to exist before
//! this file could: [`ActorInstance::set_max_hp`] writes an HP line
//! that no die produced, [`ActorInstance::set_level`] writes a
//! proficiency bonus the same way, and
//! [`AttackParams::always_hits`](crate::engine::attack::AttackParams::always_hits)
//! reads the one attack in the whole document whose printed result is
//! the word *"Automatic hit"* rather than a bonus.
//!
//! ## What is here and what is not
//!
//! The stat block's own lines all land: AC 20, the flying hover, the
//! Truesight, the necrotic and poison immunity, the seven condition
//! immunities, Incorporeal Movement (which the engine already has, at
//! [`crate::engine::incorporeal`], including the *"5 (1d10) Force
//! damage if it ends its turn inside an object"* clause the avatar
//! shares with the ghosts), and the scythe.
//!
//! What is not here belongs to the card rather than to the stat block,
//! and is named in [`crate::actions::item_actions`] where the card is:
//! *"the avatar targets only you with its attacks"*, *"if an ally of
//! yours deals damage to the avatar, that ally summons another"*, and
//! *"a creature slain by an avatar can't be restored to life"*. The
//! first is a targeting restriction the engine has no lane for — its
//! AI picks by threat, not by grudge — and the second is a spawn
//! trigger on a damage event. Both are the card's clauses; neither is
//! a line of this stat block.

use crate::actions::action_template::{Action, MELEE_REACH, TargetingSchema, first_target_id};
use crate::actions::class_features::{AVATAR_OF_DEATH_TAG, INCORPOREAL_MOVEMENT_TAG};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{Multiattack, add_flat_damage_rider};
use crate::actors::actor_template::{CreatureTemplate, damage_modifiers_from};
use crate::conditions::Condition;
use crate::engine::action_overrides::ActionOverride;
use crate::engine::attack::{AttackParams, resolve_attack_outcome};
use crate::engine::dice::Dice;
use crate::engine::encounter::EncounterInstance;
use crate::engine::side_effects::ApplicableSideEffect;
use crate::engine::types::{
    AbilityScoreType, Coordinate, CreatureType, DamageModifier, DamageType, Language, Size,
    SpecialSense,
};
use std::collections::HashSet;
use std::sync::LazyLock;

/// The scythe's swing half — *"Hit: 7 (1d8 + 3) Slashing damage"*. The
/// `+3` is the avatar's Strength modifier, which every avatar has,
/// because Strength 16 is printed rather than derived.
const SCYTHE_DICE: Dice = Dice::new(1, 8);

/// …and its necrotic half — *"plus 4 (1d8) Necrotic damage"*. A flat
/// rider on the same hit rather than a second attack, which is how the
/// Hit line reads it.
const SCYTHE_NECROTIC: Dice = Dice::new(1, 8);

/// SRD 5.2 **Reaping Scythe** — *"Melee Attack Roll: Automatic hit,
/// reach 5 ft. Hit: 7 (1d8 + 3) Slashing damage plus 4 (1d8) Necrotic
/// damage."*
///
/// A bespoke `impl Action` rather than a row on `WeaponWithRider`,
/// which is otherwise exactly this weapon's shape — a swing plus a
/// typed rider. The reason is the first two words: this is the only
/// attack in the document that does not roll, and threading an
/// `always_hits` column through a chassis that eleven other call sites
/// share would put a flag on every natural weapon in the bestiary to
/// let one of them say "no, really". The chassis is for shapes that
/// recur. This one does not.
///
/// What it does share is the pipeline. The swing goes through
/// `resolve_attack_outcome` like everything else, so it takes cover,
/// it is turned away by Sanctuary, it burns the attacker's one-shot
/// riders, it lands on a Mirror Image, and it crits against something
/// Paralyzed — and only the single question of whether the total beat
/// the armour class is answered by the stat block instead of by a die.
/// See [`AttackParams::always_hits`].
pub struct ReapingScythe;

impl Action for ReapingScythe {
    fn name(&self) -> &str {
        "reaping scythe"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["scythe", "reap"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }

    fn reach_tiles(&self) -> Option<isize> {
        Some(MELEE_REACH)
    }

    fn is_weapon_attack(&self) -> bool {
        true
    }

    fn is_melee_attack(&self) -> bool {
        true
    }

    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Slashing, DamageType::Necrotic]
    }

    /// The whole Hit line, and no hit/miss branch above it worth
    /// speaking of — the swing either arrives or was stopped by
    /// something upstream of the roll, and `resolve_attack_outcome`
    /// reports the second as an empty effect list exactly as it does
    /// for a miss.
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let damage_bonus = caster.ability_modifier(AbilityScoreType::Strength);
        let (mut effects, _dealt) = resolve_attack_outcome(
            encounter,
            AttackParams {
                caster_id,
                target_id,
                action_name: "reaping scythe",
                // Printed as a result rather than as a bonus, so there
                // is no to-hit number to carry. Zero is not a modifier
                // here; it is the absence of one, and `always_hits`
                // directly below is what makes the distinction moot.
                attack_bonus: 0,
                damage_dice: SCYTHE_DICE,
                damage_bonus,
                damage_type: DamageType::Slashing,
                always_hits: true,
                ..AttackParams::DEFAULTS
            },
        );
        // The rider rides a hit, and an empty list is the engine's
        // spelling of "the swing did not arrive" — which for this
        // weapon means Sanctuary or a redirect rather than a miss, but
        // the gate is the same one every other rider uses.
        if effects.is_empty() {
            return effects;
        }
        add_flat_damage_rider(
            encounter,
            target_id,
            SCYTHE_NECROTIC,
            DamageType::Necrotic,
            "the scythe's edge",
            &mut effects,
        );
        effects
    }

    /// Averages, not a roll: `4.5 + 3` of slashing and `4.5` of
    /// necrotic. No hit chance folded in, and that is the honest
    /// number rather than an optimistic one — the swing lands.
    fn expected_damage(&self, encounter: &EncounterInstance, caster_id: usize) -> Option<f32> {
        let bonus = encounter
            .actors
            .get(&caster_id)
            .map(|a| a.ability_modifier(AbilityScoreType::Strength))
            .unwrap_or(0) as f32;
        Some(SCYTHE_DICE.average_roll() + bonus + SCYTHE_NECROTIC.average_roll())
    }
}

/// The swing itself, for the action list and the prompt parser.
pub static REAPING_SCYTHE: ReapingScythe = ReapingScythe;

/// SRD 5.2 **Multiattack** — *"The avatar makes a number of Reaping
/// Scythe attacks equal to half the summoner's Proficiency Bonus
/// (rounded up)."*
///
/// The printed `count` is `1`, which is what the expression comes to
/// for a summoner of any level below 5, and it is deliberately the
/// *floor* of the range rather than its middle: the real count is
/// resolved per swing by
/// `EncounterInstance::attack_routine_swings`, which reads the avatar's
/// own proficiency bonus off the sheet the Deck stamped it with. That
/// is the same lane the hydra's head count already travels, and it is
/// there for the same reason — what changes during a fight is the
/// creature, not the routine.
pub static AVATAR_MULTIATTACK: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "avatar multiattack",
    sub_attack: &REAPING_SCYTHE,
    count: 1,
});

/// **Avatar of Death** — Medium Undead, Neutral evil; *"CR None (XP 0;
/// PB equals its summoner's)"*.
///
/// AC 20 and every ability score at 16, which is a stat block written
/// to be unpleasant rather than to be balanced: it is the price of a
/// card, not a monster the table was supposed to survive on merit. The
/// HP line is the one that makes it frightening in proportion — half
/// the summoner's maximum means it scales with exactly the character
/// who drew it, and a level-1 conjurer gets a nuisance while a
/// level-17 one gets something with the hit points of a giant.
///
/// The saves are *"+3"* across the board, which is every score's
/// modifier and no proficiency anywhere, so `proficient_saves` is
/// deliberately empty rather than forgotten. The Languages line —
/// *"All languages known to its summoner"* — collapses to Common,
/// because the engine's languages are a communication channel nothing
/// in a fight reads and the avatar has nothing to say.
pub static AVATAR_OF_DEATH_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&REAPING_SCYTHE);
    actions.push(&*AVATAR_MULTIATTACK);
    CreatureTemplate {
        name: "Avatar of Death",
        // 'Y' — free, and the nearest thing in the letter pool to a
        // scythe held upright. 'A' is the Air Elemental's, 'D' the
        // dragons', 'S' the spiders'.
        glyph: 'Y',
        ac: 20,
        // A placeholder, and the only line on this sheet that is a lie:
        // RAW's HP is half the summoner's maximum and the field is a
        // `Dice`. Whatever this rolls is overwritten by
        // `ActorInstance::set_max_hp` the moment the card summons one —
        // see `crate::actions::item_actions::DRAW_FROM_THE_DECK`. It is
        // written as a plausible mid-level figure rather than as `1d1`
        // so an avatar somebody drops on a board by hand (a test, the
        // debug spawner) is still a fight.
        hitpoints: "10d8".parse().unwrap(),
        // RAW: "Speed 60 ft., Fly 60 ft. (hover)". Both halves, because
        // it has both — unlike the ghosts, whose walking speed is zero.
        speed: 60.0,
        fly_speed: 60.0,
        hovers: true,
        strength: 16,
        dexterity: 16,
        constitution: 16,
        intelligence: 16,
        wisdom: 16,
        charisma: 16,
        // RAW: "Senses Truesight 60 ft." — and nothing else, which is
        // the line saying the avatar sees through everything the board
        // can put in front of it.
        senses: HashSet::from([SpecialSense::Truesight(60)]),
        languages: HashSet::from([Language::Common]),
        // "CR None (XP 0)". Zero rather than a small number, and it is
        // load-bearing: the encounter generator budgets by CR, and an
        // avatar is not something a generator should ever be able to
        // afford. Nothing but the card puts one on a board.
        cr: 0.0,
        size: Size::Medium,
        creature_type: CreatureType::Undead,
        actions,
        features: HashSet::from([
            // The stone is not a wall to this thing either — same
            // clause, same 1d10, as every ghost in the bestiary. See
            // `crate::engine::incorporeal`.
            INCORPOREAL_MOVEMENT_TAG,
            // …and the tag that tells `attack_routine_swings` to read
            // the Multiattack's count off the proficiency bonus rather
            // than off the printed number.
            AVATAR_OF_DEATH_TAG,
        ]),
        // RAW: "Immunities Necrotic, Poison; Charmed, Exhaustion,
        // Frightened, Paralyzed, Petrified, Poisoned, Unconscious."
        //
        // Pointedly *not* the shared incorporeal-undead envelope, which
        // the specter and the allip both take: that list adds Grappled,
        // Prone and Restrained, and this one does not have them. An
        // avatar can be tripped and it can be held, which is most of
        // what a party can actually do about it — and it takes full
        // damage from an ordinary sword, because the envelope's
        // bludgeoning/piercing/slashing resistance is absent too. AC 20
        // and the summoner's hit points are the whole defence.
        damage_modifiers: damage_modifiers_from([
            (DamageType::Necrotic, DamageModifier::Immunity),
            (DamageType::Poison, DamageModifier::Immunity),
        ]),
        condition_immunities: HashSet::from([
            Condition::Charmed,
            Condition::Exhausted,
            Condition::Frightened,
            Condition::Paralyzed,
            Condition::Petrified,
            Condition::Poisoned,
            Condition::Unconscious,
        ]),
        ..CreatureTemplate::defaults()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::actor_gen::ActorGenParams;
    use crate::engine::terrain_gen::TerrainGenParams;

    fn board() -> EncounterInstance {
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
        EncounterInstance::from_params(&tp, &ap, Some(7)).unwrap()
    }

    #[test]
    fn the_avatar_carries_the_lines_the_book_prints() {
        let mut e = board();
        let id = e
            .instantiate_creature(&AVATAR_OF_DEATH_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        let a = &e.actors[&id];
        assert_eq!(a.armor_class(), 20);
        assert_eq!(a.creature_type(), CreatureType::Undead);
        assert_eq!(a.size(), Size::Medium);
        for ability in [
            AbilityScoreType::Strength,
            AbilityScoreType::Dexterity,
            AbilityScoreType::Constitution,
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ] {
            assert_eq!(a.ability_score(ability), 16, "every score is 16");
            assert_eq!(a.ability_modifier(ability), 3, "and every modifier +3");
        }
        assert!(a.find_action("reaping scythe").is_some());
        assert!(a.find_action("avatar multiattack").is_some());
    }

    /// The immunity list is shorter than the one every other
    /// incorporeal undead in the bestiary takes, and the shortness is
    /// the point: an avatar can be tripped and held, and an ordinary
    /// sword hurts it.
    #[test]
    fn the_avatar_declines_the_incorporeal_undead_envelope() {
        let mut e = board();
        let id = e
            .instantiate_creature(&AVATAR_OF_DEATH_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        let a = &e.actors[&id];
        assert_eq!(
            a.damage_modifier(DamageType::Necrotic),
            Some(DamageModifier::Immunity)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Poison),
            Some(DamageModifier::Immunity)
        );
        assert_eq!(
            a.nonmagical_damage_modifier(DamageType::Slashing),
            None,
            "a steel blade hurts it in full"
        );
        assert!(a.effectively_immune_to_condition(Condition::Charmed));
        assert!(a.effectively_immune_to_condition(Condition::Frightened));
        assert!(
            !a.effectively_immune_to_condition(Condition::Prone),
            "RAW's list does not include Prone"
        );
        assert!(
            !a.effectively_immune_to_condition(Condition::Restrained),
            "nor Restrained"
        );
        assert!(
            !a.effectively_immune_to_condition(Condition::Grappled),
            "nor Grappled"
        );
    }

    /// *"Melee Attack Roll: Automatic hit."* The target here is a
    /// plate-armoured knight, which the avatar has no attack bonus to
    /// beat AC 18 with — so the only way every swing lands is the flag.
    #[test]
    fn the_scythe_lands_on_plate_every_single_time() {
        use crate::actors::creatures::knights::KNIGHT_TEMPLATE;

        for seed in 0..12u64 {
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
            let mut e = EncounterInstance::from_params(&tp, &ap, Some(seed)).unwrap();
            let avatar = e
                .instantiate_creature(&AVATAR_OF_DEATH_TEMPLATE, Coordinate::new(5, 5), 0, 0)
                .unwrap();
            let knight = e
                .instantiate_creature(&KNIGHT_TEMPLATE, Coordinate::new(7, 5), 1, 0)
                .unwrap();
            let before = e.actors[&knight].hitpoints();
            let effects = REAPING_SCYTHE.side_effects(&mut e, avatar, Some(&vec![knight]), None, None);
            assert!(
                effects.len() >= 2,
                "seed {seed}: the slashing half and the necrotic half both arrive"
            );
            for ef in effects {
                ef.apply(&mut e);
            }
            assert!(
                e.actors[&knight].hitpoints() < before,
                "seed {seed}: an automatic hit is not a coin flip"
            );
        }
    }
}
