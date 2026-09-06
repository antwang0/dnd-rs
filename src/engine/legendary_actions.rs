//! 5e **legendary actions** — what a boss does between everyone else's
//! turns.
//!
//! ```text
//! Legendary Actions
//! The dragon can take 3 legendary actions, choosing from the options
//! below. Only one legendary action option can be used at a time and
//! only at the end of another creature's turn. The dragon regains spent
//! legendary actions at the start of its turn.
//! ```
//!
//! That paragraph is the entire reason a solo boss fight works at a
//! table: without it a dragon facing four adventurers acts once for
//! every four times they do, and the fight is a firing squad. The
//! engine already had the *budget* — `legendary_actions_per_round` on
//! the template, `Resource::LegendaryAction` in the resource enum,
//! `legendary_action_slots` refreshed by `reset_for_new_round`, a UI
//! label for it, and an "out of legendary actions" refusal string — and
//! it had no options to spend it on and nothing that ever spent it. Its
//! own stat blocks said so out loud: the androsphinx's docstring noted
//! that "the `Resource::LegendaryAction` lane is wired but unused", and
//! the kraken's that "the LA budget in practice translates to extra
//! tentacle swings between PCs' turns" — which it did not, because
//! nothing swung.
//!
//! Eleven templates carried the budget. This module is what they spend
//! it on.
//!
//! ## Why an option is a function and not an `Action`
//!
//! For the same reason a lair action is — see
//! `crate::engine::lair_actions`, whose shape this module deliberately
//! mirrors down to the field names. An `Action` is built around a
//! chooser: a caster, a cost the turn's economy understands, a
//! targeting schema, and a validation pass over arguments somebody
//! supplied. A legendary action has a cost the turn economy does *not*
//! understand (it is paid out of a separate pool, on somebody else's
//! turn), and nobody supplies it arguments — RAW's text picks its own
//! victims ("each creature within 10 feet of the dragon", "one creature
//! the lich can see").
//!
//! So an option is a name, a price in legendary-action points, and a
//! function that reads the board. The difference from a lair action is
//! the price and who it belongs to: a lair action is the *place*
//! acting and costs nothing, a legendary action is the *creature*
//! acting and costs it something it can run out of.
//!
//! ## When they fire
//!
//! At the end of every turn that is not the legendary creature's own —
//! `EncounterInstance::dispatch_legendary_actions`, called from
//! `advance_initiative` with the id of the turn that just closed. One
//! option per creature per turn-end, which is RAW's "only one legendary
//! action option can be used at a time".
//!
//! ## What is deliberately not modeled
//!
//! **The options with no combat surface.** Every dragon's list opens
//! with "Detect: The dragon makes a Wisdom (Perception) check", and the
//! engine rolls no Perception checks — see the note in
//! `crate::engine::lighting`. An option that resolved to nothing would
//! still cost a point, so it would be a rule that made the dragon
//! *worse*; it is left off the list instead.
//!
//! **Choice.** The dispatcher draws uniformly from the options the
//! creature can currently afford rather than picking the best one. That
//! is the same policy `dispatch_lair_actions` uses and it is deliberate:
//! the alternative is a second AI, scoring options against a board, and
//! the `SimpleAi` that would have to grow it does not run between
//! turns. A dragon that sometimes tail-swipes when it could have winged
//! is a dragon playing slightly below its ceiling, which is a much
//! smaller lie than a dragon that never acts at all.

use crate::actions::action_template::{
    apply_burst_save_condition, apply_enemy_burst_save_damage,
};
use crate::engine::attack::{AttackParams, resolve_attack_outcome};
use crate::conditions::{Condition, ConditionTimer};
use crate::engine::dice::Dice;
use crate::engine::encounter::EncounterInstance;
use crate::engine::saves::SaveDamagePolicy;
use crate::engine::side_effects::{ApplicableSideEffect, DealDamage};
use crate::engine::types::{AbilityScoreType, DamageType};

/// One entry in a creature's legendary repertoire.
///
/// `fire` takes the encounter and the id of the creature spending the
/// points — the same signature `LairAction::fire` has, and for the same
/// reason: the effect picks its own targets off the board rather than
/// being handed them.
pub struct LegendaryAction {
    /// Shown in the log when this option is taken.
    pub name: &'static str,
    /// Price in legendary-action points, out of the creature's
    /// `legendary_actions_per_round` budget. RAW prints "(Costs 2
    /// Actions)" beside the expensive options and nothing beside the
    /// cheap ones, which is a `1`.
    ///
    /// A `0` here would be an option that never runs out, which is not
    /// a thing 5e has; the dispatcher treats the whole repertoire as
    /// unaffordable rather than looping, but the honest fix is not to
    /// write one.
    pub cost: u32,
    /// Resolve the effect. Applies its own side effects directly, the
    /// way the reaction dispatcher and the lair dispatcher both do —
    /// there is no turn for it to be part of and no stack to queue on.
    pub fire: fn(&mut EncounterInstance, usize),
}

/// The DC a legendary option rolls against.
///
/// Identical in shape to `lair_actions::lair_dc` and separate from it
/// on purpose: the two answer the same question for different things
/// (the creature's own save DC versus the place's), they take different
/// floors, and folding them together would mean a martial legendary
/// creature's gaze and its cave were guaranteed to be equally
/// dangerous. The floor is what keeps a tarrasque's stare frightening
/// when the tarrasque has no casting ability to derive a DC from.
fn legendary_dc(encounter: &EncounterInstance, actor_id: usize) -> i32 {
    encounter
        .actors
        .get(&actor_id)
        .map(|a| {
            a.best_spell_save_dc([
                AbilityScoreType::Charisma,
                AbilityScoreType::Intelligence,
                AbilityScoreType::Wisdom,
                AbilityScoreType::Constitution,
            ])
        })
        .unwrap_or(LEGENDARY_DC_FLOOR)
        .max(LEGENDARY_DC_FLOOR)
}

/// Floor for `legendary_dc`. Legendary creatures start around CR 10 and
/// the DCs their stat blocks print start around here.
const LEGENDARY_DC_FLOOR: i32 = 15;

/// The nearest enemy within `reach` tiles that the actor can see, or
/// `None`.
///
/// Reach is measured footprint to footprint, which for the creatures on
/// this list is the whole point: a kraken's tentacle reaches 30 ft from
/// its body, not from the corner tile its anchor happens to sit on.
/// Ties break on the lower id so a seeded replay makes the same choice,
/// the same as every other single-target sweep in the engine.
fn nearest_enemy_within(
    encounter: &EncounterInstance,
    actor_id: usize,
    reach: isize,
) -> Option<usize> {
    let origin = encounter.actors.get(&actor_id)?.location();
    let mut candidates: Vec<(isize, usize)> = encounter
        // A generous candidate sweep, narrowed to true footprint reach
        // below: the burst filter is what knows about teams and about
        // who is still standing.
        .enemy_burst_targets(actor_id, origin, LEGENDARY_SEARCH)
        .into_iter()
        .filter(|&id| encounter.actor_has_line_of_sight(actor_id, id))
        .filter_map(|id| {
            let dist = encounter.footprint_distance(actor_id, id)?;
            (dist <= reach).then_some((dist, id))
        })
        .collect();
    candidates.sort_unstable();
    candidates.first().map(|&(_, id)| id)
}

/// How far the candidate sweep in `nearest_enemy_within` looks before
/// the real reach test narrows it. Wide enough to cover a generated map
/// end to end, so the filter that matters is the reach and not this.
const LEGENDARY_SEARCH: isize = 40;

/// What kind of attack roll a legendary option makes.
///
/// Two independent yes/no questions — is it melee, is it a spell — and
/// three of the four combinations occur on these lists, which is why
/// this is an enum rather than two booleans on the struct below: the
/// fourth combination (a ranged *weapon* attack) has no entry, and a
/// pair of bools would let a future option ask for one by accident.
///
/// Both answers reach real rules. `is_melee` drives the prone-target
/// clause in `compute_attack_mode`, which is inverted between the two
/// — a swing at a prone creature has advantage and a shot at one has
/// disadvantage — so a ray resolved as a melee swing is a ray that
/// gets *better* the more of the party is on the floor. `is_spell` is
/// what marks an attack roll as magic for the riders that ask.
#[derive(Debug, Clone, Copy)]
enum SwingKind {
    /// A limb: the dragon's tail, the kraken's tentacle, the vampire's
    /// fist.
    Melee,
    /// A melee *spell* attack — the lich's Paralyzing Touch, which RAW
    /// rolls at touch range off its spellcasting ability.
    MeleeSpell,
    /// A ranged spell attack: the beholder's eye ray.
    RangedSpell,
}

impl SwingKind {
    const fn is_melee(self) -> bool {
        matches!(self, SwingKind::Melee | SwingKind::MeleeSpell)
    }
    const fn is_spell(self) -> bool {
        matches!(self, SwingKind::MeleeSpell | SwingKind::RangedSpell)
    }
}

/// The facts that describe one legendary swing.
///
/// A struct rather than six positional parameters threaded through
/// three helpers, for the reason `attack::HitContext` gives for being
/// one: `legendary_attack_with_grab(e, id, "chomp", Strength,
/// Dice::new(4, 12), Piercing, 6, Melee, Grappled, Permanent)` is a
/// call nobody can read at the site, and the two riders below would
/// each have carried the whole list again.
struct Swing {
    /// Names the option in the log and on the attack roll.
    name: &'static str,
    /// Drives both the attack roll and the damage bonus — every swing
    /// on these lists is a monster's own physical or psionic reach, so
    /// the two never come from different scores.
    ability: AbilityScoreType,
    dice: Dice,
    damage_type: DamageType,
    /// Footprint-to-footprint reach in tiles.
    reach: isize,
    kind: SwingKind,
}

/// One swing of a legendary melee option at the nearest enemy in reach.
/// Returns the creature it landed on, or `None` if nobody was in reach
/// or the swing missed.
///
/// Routed through `weapon_swing_with_damage` rather than an open-coded
/// d20 so the swing picks up everything a swing is supposed to: cover,
/// Bless and Bane, Sanctuary, Mirror Image, the reactive damage clamps,
/// Multiattack Defense, and the attacker's own buffs. A boss that swung
/// outside the pipeline would be a boss whose tail ignored the paladin's
/// aura.
fn legendary_attack(
    encounter: &mut EncounterInstance,
    actor_id: usize,
    swing: &Swing,
) -> Option<usize> {
    let target_id = nearest_enemy_within(encounter, actor_id, swing.reach)?;
    let actor = encounter.actors.get(&actor_id)?;
    let attack_bonus = actor.spell_attack_modifier(swing.ability);
    let damage_bonus = actor.ability_modifier(swing.ability);
    let (effects, _damage) = resolve_attack_outcome(
        encounter,
        AttackParams {
            caster_id: actor_id,
            target_id,
            action_name: swing.name,
            attack_bonus,
            damage_dice: swing.dice,
            damage_bonus,
            damage_type: swing.damage_type,
            is_melee: swing.kind.is_melee(),
            // No long-range falloff: RAW prints one reach per legendary
            // option and no second band beyond it, so there is no
            // threshold for a shot to cross.
            long_range: None,
            // Only the lance minds being crowded, and nothing on these
            // lists is one.
            min_range: None,
            is_spell: swing.kind.is_spell(),
        },
    );
    let landed = !effects.is_empty();
    for effect in effects {
        effect.apply(encounter);
    }
    landed.then_some(target_id)
}

/// `legendary_attack`, plus a condition on the creature the swing
/// connects with — the grapples and grabs that half of these stat
/// blocks hang off a hit.
fn legendary_attack_with_grab(
    encounter: &mut EncounterInstance,
    actor_id: usize,
    swing: &Swing,
    condition: Condition,
    timer: ConditionTimer,
) {
    let Some(target_id) = legendary_attack(encounter, actor_id, swing) else {
        return;
    };
    // The swing can have dropped them. A corpse is not grappled.
    if !encounter
        .actors
        .get(&target_id)
        .is_some_and(|a| a.is_combat_active())
    {
        return;
    }
    if encounter.actor_immune_to_condition(target_id, condition) {
        return;
    }
    let name = encounter.actor_name(target_id);
    if let Some(a) = encounter.actors.get_mut(&target_id) {
        a.add_condition(condition, timer);
    }
    encounter.log(format!("  {}: {} is {}.", swing.name, name, condition.name()));
}

/// "Each creature within N feet of the creature must make a save."
/// Centred on the actor rather than on a chosen point, which is what
/// every legendary burst in the rules says and the one place this
/// differs from the lair layer — a lair erupts *somewhere*, a wing beat
/// happens where the wings are.
fn legendary_burst_damage(
    encounter: &mut EncounterInstance,
    actor_id: usize,
    radius: isize,
    save: AbilityScoreType,
    dice: Dice,
    damage_type: DamageType,
    policy: SaveDamagePolicy,
) {
    let Some(center) = encounter.actors.get(&actor_id).map(|a| a.location()) else {
        return;
    };
    let dc = legendary_dc(encounter, actor_id);
    apply_enemy_burst_save_damage(
        encounter,
        actor_id,
        center,
        radius,
        save,
        dc,
        dice,
        damage_type,
        policy,
    );
}

/// The condition half of `legendary_burst_damage`, centred the same way.
fn legendary_burst_condition(
    encounter: &mut EncounterInstance,
    actor_id: usize,
    radius: isize,
    save: AbilityScoreType,
    condition: Condition,
    timer: ConditionTimer,
) {
    let Some(center) = encounter.actors.get(&actor_id).map(|a| a.location()) else {
        return;
    };
    let dc = legendary_dc(encounter, actor_id);
    apply_burst_save_condition(
        encounter, actor_id, center, radius, save, dc, condition, timer,
    );
}

/// A single enemy in sight makes a save or takes a condition — the
/// gaze options ("one creature the lich can see within 10 feet of it").
fn legendary_gaze(
    encounter: &mut EncounterInstance,
    actor_id: usize,
    option_name: &'static str,
    reach: isize,
    save: AbilityScoreType,
    condition: Condition,
    timer: ConditionTimer,
) {
    let Some(target_id) = nearest_enemy_within(encounter, actor_id, reach) else {
        return;
    };
    let dc = legendary_dc(encounter, actor_id);
    let target_name = encounter.actor_name(target_id);
    if encounter
        .roll_save_against_caster(target_id, save, dc, actor_id)
        .passed()
    {
        return;
    }
    if encounter.actor_immune_to_condition(target_id, condition) {
        encounter.log(format!("  {}: {} is unaffected.", option_name, target_name));
        return;
    }
    if let Some(a) = encounter.actors.get_mut(&target_id) {
        a.add_condition(condition, timer);
    }
    encounter.log(format!(
        "  {}: {} is {}.",
        option_name,
        target_name,
        condition.name()
    ));
}

/// A ranged attack that drains the attacker's target and feeds the
/// attacker — the vampire's bite, and the one option on these lists
/// whose whole point is that the boss heals off it.
fn legendary_drain(
    encounter: &mut EncounterInstance,
    actor_id: usize,
    swing: &Swing,
    drain: Dice,
) {
    let Some(target_id) = legendary_attack(encounter, actor_id, swing) else {
        return;
    };
    let drained = encounter.roll(&drain);
    let target_name = encounter.actor_name(target_id);
    let self_name = encounter.actor_name(actor_id);
    encounter.log(format!(
        "  {}: {} drinks {} from {}.",
        swing.name, self_name, drained, target_name
    ));
    DealDamage {
        actor_id: target_id,
        amount: drained,
        damage_type: DamageType::Necrotic,
    }
    .apply(encounter);
    crate::engine::side_effects::Heal {
        actor_id,
        amount: drained,
    }
    .apply(encounter);
}

/// RAW's "the vampire moves up to its speed without provoking
/// opportunity attacks."
///
/// Walked one step at a time through `step_toward_actor` — the
/// pathfinder that already knows how to route a footprint around bad
/// ground — and landed with `place_actor_at` rather than
/// `walk_actor_to`, which is exactly what buys the no-provoking half:
/// `walk_actor_to` is the lane opportunity attacks hang off, and a
/// legendary stride is by RAW not on it. The zone triggers *are* still
/// asked on every step, because "without provoking opportunity attacks"
/// is a clause about reactions and says nothing about walking into a
/// web.
fn legendary_stride(encounter: &mut EncounterInstance, actor_id: usize, tiles: usize) {
    let Some(target_id) = nearest_enemy_within(encounter, actor_id, LEGENDARY_SEARCH) else {
        return;
    };
    let mut moved = 0;
    for _ in 0..tiles {
        let Some(step) = encounter.step_toward_actor(actor_id, target_id) else {
            break;
        };
        if encounter.place_actor_at(actor_id, step).is_err() {
            break;
        }
        moved += 1;
        encounter.touch_zones(actor_id);
        encounter.charge_zone_movement(actor_id);
        if !encounter
            .actors
            .get(&actor_id)
            .is_some_and(|a| a.is_combat_active())
        {
            return;
        }
    }
    if moved > 0 {
        let name = encounter.actor_name(actor_id);
        encounter.log(format!("  {} shifts {} tiles unopposed.", name, moved));
    }
}

/// Ordinary 5-ft reach, under the name the rest of this file measures
/// in. Aliased rather than spelled out at each site so a swing's reach
/// reads as a number beside the other numbers.
const MELEE: isize = crate::actions::action_template::MELEE_REACH;
/// Reach of a Huge or Gargantuan boss's secondary limb — 15 ft, the
/// reach every dragon tail and sphinx claw in this file prints.
const REACH_LONG: isize = 6;
/// The 10-ft radius RAW's "each creature within 10 feet" burst options
/// use.
const RADIUS_CLOSE: isize = 4;
/// The 20-ft radius the wider legendary bursts use (Disrupt Life,
/// Searing Burst).
const RADIUS_WIDE: isize = 8;

/// **Dragon** (MM, adult chromatic dragons). Tail and wings — the two
/// options on every dragon's list that have a combat surface. Detect is
/// dropped; see the module note.
pub const DRAGON_LEGENDARY: &[LegendaryAction] = &[
    LegendaryAction {
        name: "tail attack",
        cost: 1,
        fire: |e, id| {
            legendary_attack(
                e,
                id,
                &Swing {
                    name: "tail attack",
                    ability: AbilityScoreType::Strength,
                    dice: Dice::new(2, 8),
                    damage_type: DamageType::Bludgeoning,
                    reach: REACH_LONG,
                    kind: SwingKind::Melee,
                },
            );
        },
    },
    LegendaryAction {
        name: "wing attack",
        cost: 2,
        fire: |e, id| {
            // RAW: "each creature within 10 feet must succeed on a
            // Dexterity saving throw or take bludgeoning damage and be
            // knocked prone." The damage and the knockdown ride the
            // same failed save, so the two sweeps below are one clause
            // resolved twice rather than two clauses — the second is
            // charged only to whoever the first caught.
            legendary_burst_damage(
                e,
                id,
                RADIUS_CLOSE,
                AbilityScoreType::Dexterity,
                Dice::new(2, 6),
                DamageType::Bludgeoning,
                SaveDamagePolicy::NoneOnSave,
            );
            legendary_burst_condition(
                e,
                id,
                RADIUS_CLOSE,
                AbilityScoreType::Dexterity,
                Condition::Prone,
                ConditionTimer::Permanent,
            );
        },
    },
];

/// **Lich** (MM). Paralyzing Touch, Frightening Gaze, Disrupt Life —
/// the three options whose text the engine can resolve. The Cantrip
/// option is dropped: spending a legendary action to cast one of the
/// lich's own spells would need the turn AI to choose and aim it, and
/// the dispatcher runs between turns where no AI does.
pub const LICH_LEGENDARY: &[LegendaryAction] = &[
    LegendaryAction {
        name: "paralyzing touch",
        cost: 2,
        fire: |e, id| {
            let dc = legendary_dc(e, id);
            let Some(target_id) = legendary_attack(
                e,
                id,
                &Swing {
                    name: "paralyzing touch",
                    ability: AbilityScoreType::Intelligence,
                    dice: Dice::new(3, 6),
                    damage_type: DamageType::Cold,
                    reach: MELEE,
                    kind: SwingKind::MeleeSpell,
                },
            ) else {
                return;
            };
            if !e
                .actors
                .get(&target_id)
                .is_some_and(|a| a.is_combat_active())
            {
                return;
            }
            if e.roll_save_against_caster(target_id, AbilityScoreType::Constitution, dc, id)
                .passed()
            {
                return;
            }
            if e.actor_immune_to_condition(target_id, Condition::Paralyzed) {
                return;
            }
            let name = e.actor_name(target_id);
            if let Some(a) = e.actors.get_mut(&target_id) {
                a.add_condition(Condition::Paralyzed, ConditionTimer::Rounds(3));
            }
            e.log(format!("  paralyzing touch: {} seizes up.", name));
        },
    },
    LegendaryAction {
        name: "frightening gaze",
        cost: 2,
        fire: |e, id| {
            legendary_gaze(
                e,
                id,
                "frightening gaze",
                RADIUS_WIDE,
                AbilityScoreType::Wisdom,
                Condition::Frightened,
                ConditionTimer::Rounds(3),
            )
        },
    },
    LegendaryAction {
        name: "disrupt life",
        cost: 3,
        fire: |e, id| {
            legendary_burst_damage(
                e,
                id,
                RADIUS_WIDE,
                AbilityScoreType::Constitution,
                Dice::new(6, 6),
                DamageType::Necrotic,
                SaveDamagePolicy::HalfOnSave,
            )
        },
    },
];

/// **Beholder** (MM). One option, taken three times a round: "the
/// beholder uses one random eye ray." The engine's beholder has a
/// single force-typed ray rather than ten separately-named ones, so the
/// randomness RAW asks for lives in which enemy the ray finds rather
/// than in which ray fires.
pub const BEHOLDER_LEGENDARY: &[LegendaryAction] = &[LegendaryAction {
    name: "eye ray",
    cost: 1,
    fire: |e, id| {
        legendary_attack(
            e,
            id,
            &Swing {
                name: "eye ray",
                ability: AbilityScoreType::Intelligence,
                dice: Dice::new(4, 8),
                damage_type: DamageType::Force,
                reach: LEGENDARY_SEARCH,
                kind: SwingKind::RangedSpell,
            },
        );
    },
}];

/// **Kraken** (MM). The tentacle swing the kraken's own docstring
/// promised its legendary budget would translate into, and the
/// lightning storm beside it. Fling is still absent — the engine has no
/// thrown-creature lane — and is now absent from a list rather than
/// from an unspent budget.
pub const KRAKEN_LEGENDARY: &[LegendaryAction] = &[
    LegendaryAction {
        name: "tentacle attack",
        cost: 1,
        fire: |e, id| {
            legendary_attack_with_grab(
                e,
                id,
                &Swing {
                    name: "tentacle attack",
                    ability: AbilityScoreType::Strength,
                    dice: Dice::new(3, 6),
                    damage_type: DamageType::Bludgeoning,
                    // 30 ft of tentacle.
                    reach: 12,
                    kind: SwingKind::Melee,
                },
                Condition::Grappled,
                ConditionTimer::Permanent,
            )
        },
    },
    LegendaryAction {
        name: "lightning storm",
        cost: 2,
        fire: |e, id| {
            legendary_burst_damage(
                e,
                id,
                RADIUS_WIDE,
                AbilityScoreType::Dexterity,
                Dice::new(4, 10),
                DamageType::Lightning,
                SaveDamagePolicy::HalfOnSave,
            )
        },
    },
];

/// **Vampire** (MM). Move, Unarmed Strike, Bite — the list as printed,
/// and the one repertoire where the cheap option that moves is worth as
/// much as the one that swings: a vampire that can reposition between
/// turns is a vampire that is never where the party left it.
pub const VAMPIRE_LEGENDARY: &[LegendaryAction] = &[
    LegendaryAction {
        name: "move",
        cost: 1,
        // "Up to its speed" — a vampire's 30 ft is 12 tiles.
        fire: |e, id| legendary_stride(e, id, 12),
    },
    LegendaryAction {
        name: "unarmed strike",
        cost: 1,
        fire: |e, id| {
            legendary_attack_with_grab(
                e,
                id,
                &Swing {
                    name: "unarmed strike",
                    ability: AbilityScoreType::Strength,
                    dice: Dice::new(1, 8),
                    damage_type: DamageType::Bludgeoning,
                    reach: MELEE,
                    kind: SwingKind::Melee,
                },
                Condition::Grappled,
                ConditionTimer::Permanent,
            )
        },
    },
    LegendaryAction {
        name: "bite",
        cost: 2,
        fire: |e, id| {
            legendary_drain(
                e,
                id,
                &Swing {
                    name: "bite",
                    ability: AbilityScoreType::Strength,
                    dice: Dice::new(1, 6),
                    damage_type: DamageType::Piercing,
                    reach: MELEE,
                    kind: SwingKind::Melee,
                },
                Dice::new(3, 6),
            )
        },
    },
];

/// **Tarrasque** (MM). Attack, Move, Chomp — the whole list is "it hits
/// you again", which is what a tarrasque is.
pub const TARRASQUE_LEGENDARY: &[LegendaryAction] = &[
    LegendaryAction {
        name: "claw",
        cost: 1,
        fire: |e, id| {
            legendary_attack(
                e,
                id,
                &Swing {
                    name: "claw",
                    ability: AbilityScoreType::Strength,
                    dice: Dice::new(4, 8),
                    damage_type: DamageType::Slashing,
                    reach: REACH_LONG,
                    kind: SwingKind::Melee,
                },
            );
        },
    },
    LegendaryAction {
        name: "move",
        cost: 1,
        // 40 ft.
        fire: |e, id| legendary_stride(e, id, 16),
    },
    LegendaryAction {
        name: "chomp",
        cost: 2,
        fire: |e, id| {
            legendary_attack_with_grab(
                e,
                id,
                &Swing {
                    name: "chomp",
                    ability: AbilityScoreType::Strength,
                    dice: Dice::new(4, 12),
                    damage_type: DamageType::Piercing,
                    reach: REACH_LONG,
                    kind: SwingKind::Melee,
                },
                Condition::Grappled,
                ConditionTimer::Permanent,
            )
        },
    },
];

/// **Pit fiend** (MM). The engine's pit fiend carries the legendary
/// budget its template declares; the options here are built from its
/// own stat block — the claw it already swings, and the hellfire its
/// whole kit is about — rather than from a printed legendary list.
pub const PIT_FIEND_LEGENDARY: &[LegendaryAction] = &[
    LegendaryAction {
        name: "claw",
        cost: 1,
        fire: |e, id| {
            legendary_attack(
                e,
                id,
                &Swing {
                    name: "claw",
                    ability: AbilityScoreType::Strength,
                    dice: Dice::new(2, 8),
                    damage_type: DamageType::Slashing,
                    reach: MELEE,
                    kind: SwingKind::Melee,
                },
            );
        },
    },
    LegendaryAction {
        name: "hellfire",
        cost: 2,
        fire: |e, id| {
            legendary_burst_damage(
                e,
                id,
                RADIUS_CLOSE,
                AbilityScoreType::Dexterity,
                Dice::new(4, 6),
                DamageType::Fire,
                SaveDamagePolicy::HalfOnSave,
            )
        },
    },
];

/// **Solar** (MM). Teleport, Searing Burst, Blinding Gaze. The teleport
/// is the stride — the engine's stride lands the solar next to whoever
/// it is judging, which is what a 120-ft teleport on a celestial is
/// for.
pub const SOLAR_LEGENDARY: &[LegendaryAction] = &[
    LegendaryAction {
        name: "teleport",
        cost: 1,
        fire: |e, id| legendary_stride(e, id, 16),
    },
    LegendaryAction {
        name: "searing burst",
        cost: 2,
        fire: |e, id| {
            legendary_burst_damage(
                e,
                id,
                RADIUS_WIDE,
                AbilityScoreType::Dexterity,
                Dice::new(6, 6),
                DamageType::Radiant,
                SaveDamagePolicy::HalfOnSave,
            )
        },
    },
    LegendaryAction {
        name: "blinding gaze",
        cost: 3,
        fire: |e, id| {
            legendary_gaze(
                e,
                id,
                "blinding gaze",
                RADIUS_WIDE,
                AbilityScoreType::Constitution,
                Condition::Blinded,
                ConditionTimer::Rounds(2),
            )
        },
    },
];

/// **Androsphinx** (MM). Claw Attack, Teleport, Roar. The roar is the
/// third of the sphinx's three, and the one its own docstring called
/// the load-bearing clause.
pub const ANDROSPHINX_LEGENDARY: &[LegendaryAction] = &[
    LegendaryAction {
        name: "claw attack",
        cost: 1,
        fire: |e, id| {
            legendary_attack(
                e,
                id,
                &Swing {
                    name: "claw attack",
                    ability: AbilityScoreType::Strength,
                    dice: Dice::new(2, 10),
                    damage_type: DamageType::Slashing,
                    reach: REACH_LONG,
                    kind: SwingKind::Melee,
                },
            );
        },
    },
    LegendaryAction {
        name: "teleport",
        cost: 2,
        fire: |e, id| legendary_stride(e, id, 16),
    },
    LegendaryAction {
        name: "roar",
        cost: 3,
        fire: |e, id| {
            legendary_burst_condition(
                e,
                id,
                RADIUS_WIDE,
                AbilityScoreType::Wisdom,
                Condition::Frightened,
                ConditionTimer::Rounds(2),
            )
        },
    },
];

/// A saving throw that ages you — the Sphinx of Lore's Weight of Years,
/// and the only legendary option in the book whose payload is a level
/// of exhaustion.
///
/// Exhaustion is not a `Condition` in this engine, it is a counter with
/// six rungs, so `legendary_gaze` cannot carry it: that helper installs
/// a condition and this one raises a number. What they share is the
/// shape RAW gives both — nearest enemy in range, one save against the
/// creature's own DC, nothing on a success — so the two sit beside each
/// other rather than one pretending to be the other.
fn legendary_exhaust(
    encounter: &mut EncounterInstance,
    actor_id: usize,
    option_name: &'static str,
    reach: isize,
    save: AbilityScoreType,
) {
    let Some(target_id) = nearest_enemy_within(encounter, actor_id, reach) else {
        return;
    };
    let dc = legendary_dc(encounter, actor_id);
    let target_name = encounter.actor_name(target_id);
    if encounter
        .roll_save_against_caster(target_id, save, dc, actor_id)
        .passed()
    {
        return;
    }
    let level = encounter
        .actors
        .get_mut(&target_id)
        .map(|a| a.gain_exhaustion(1))
        .unwrap_or(0);
    encounter.log(format!(
        "  {}: {} ages, and is exhausted (level {}).",
        option_name, target_name, level
    ));
}

/// **Sphinx of Lore** (SRD 5.2). Two options, and RAW prices the
/// interesting one at the same single point as the boring one.
///
/// **Arcane Prowl** is a teleport *and* a claw for one point, which is
/// the whole reason the sphinx is hard to pin: it takes its three claws
/// on its turn, then steps thirty feet and takes a fourth between
/// somebody else's. The teleport half collapses to a stride here — the
/// engine has no blink lane, and a sphinx that walks the same thirty
/// feet arrives in the same place, minus the ability to cross a wall.
///
/// **Weight of Years** is the exhaustion tax, and RAW's rider is that
/// the sphinx cannot use it again until the start of its next turn —
/// which the one-per-round budget already enforces here, since taking
/// it twice would need two points and the second use would be the
/// cheaper prowl instead. Written at cost 1 like its sibling, so the
/// sphinx's three points buy three interventions a round in whatever
/// mix the dispatcher likes.
pub const SPHINX_OF_LORE_LEGENDARY: &[LegendaryAction] = &[
    LegendaryAction {
        name: "arcane prowl",
        cost: 1,
        fire: |e, id| {
            // 30 ft of teleport = 12 tiles, then the claw RAW hands it
            // in the same breath.
            legendary_stride(e, id, 12);
            legendary_attack(
                e,
                id,
                &Swing {
                    name: "arcane prowl",
                    ability: AbilityScoreType::Strength,
                    dice: Dice::new(3, 6),
                    damage_type: DamageType::Slashing,
                    reach: MELEE,
                    kind: SwingKind::Melee,
                },
            );
        },
    },
    LegendaryAction {
        name: "weight of years",
        cost: 1,
        fire: |e, id| {
            legendary_exhaust(
                e,
                id,
                "weight of years",
                // RAW 120 ft = 48 tiles, wider than any board here.
                48,
                AbilityScoreType::Constitution,
            )
        },
    },
];

/// **Death knight** (MM). Built from its own stat block, like the pit
/// fiend's: the longsword it already carries, and the hellfire it
/// already throws, priced against the budget its template declares.
pub const DEATH_KNIGHT_LEGENDARY: &[LegendaryAction] = &[
    LegendaryAction {
        name: "longsword",
        cost: 1,
        fire: |e, id| {
            legendary_attack(
                e,
                id,
                &Swing {
                    name: "longsword",
                    ability: AbilityScoreType::Strength,
                    dice: Dice::new(2, 8),
                    damage_type: DamageType::Slashing,
                    reach: MELEE,
                    kind: SwingKind::Melee,
                },
            );
        },
    },
    LegendaryAction {
        name: "hellfire blast",
        cost: 2,
        fire: |e, id| {
            legendary_burst_damage(
                e,
                id,
                RADIUS_CLOSE,
                AbilityScoreType::Dexterity,
                Dice::new(4, 6),
                DamageType::Fire,
                SaveDamagePolicy::HalfOnSave,
            )
        },
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::CreatureTemplate;
    use crate::actors::creatures::androsphinxes::ANDROSPHINX_TEMPLATE;
    use crate::actors::creatures::beholders::BEHOLDER_TEMPLATE;
    use crate::actors::creatures::death_knights::DEATH_KNIGHT_TEMPLATE;
    use crate::actors::creatures::dragons::{
        ADULT_RED_DRAGON_TEMPLATE, ANCIENT_BLUE_DRAGON_TEMPLATE,
    };
    use crate::actors::creatures::goblins::GOBLIN_TEMPLATE;
    use crate::actors::creatures::krakens::KRAKEN_TEMPLATE;
    use crate::actors::creatures::liches::LICH_TEMPLATE;
    use crate::actors::creatures::pit_fiends::PIT_FIEND_TEMPLATE;
    use crate::actors::creatures::solars::SOLAR_TEMPLATE;
    use crate::actors::creatures::sphinxes_of_lore::SPHINX_OF_LORE_TEMPLATE;
    use crate::actors::creatures::tarrasques::TARRASQUE_TEMPLATE;
    use crate::actors::creatures::vampires::VAMPIRE_TEMPLATE;
    use crate::engine::actor_gen::ActorGenParams;
    use crate::engine::terrain_gen::TerrainGenParams;
    use crate::engine::types::Coordinate;

    /// Every repertoire, paired with a template that carries it. The
    /// pairing is what the sweeps below need, and writing it down once
    /// is what stops a repertoire being added with nothing pointing at
    /// it — the failure mode where a boss keeps a budget it can never
    /// spend, which is the exact state this module was written to end.
    fn repertoires() -> Vec<(&'static [LegendaryAction], &'static CreatureTemplate)> {
        vec![
            (DRAGON_LEGENDARY, &ADULT_RED_DRAGON_TEMPLATE),
            (DRAGON_LEGENDARY, &ANCIENT_BLUE_DRAGON_TEMPLATE),
            (LICH_LEGENDARY, &LICH_TEMPLATE),
            (BEHOLDER_LEGENDARY, &BEHOLDER_TEMPLATE),
            (KRAKEN_LEGENDARY, &KRAKEN_TEMPLATE),
            (VAMPIRE_LEGENDARY, &VAMPIRE_TEMPLATE),
            (TARRASQUE_LEGENDARY, &TARRASQUE_TEMPLATE),
            (PIT_FIEND_LEGENDARY, &PIT_FIEND_TEMPLATE),
            (SOLAR_LEGENDARY, &SOLAR_TEMPLATE),
            (ANDROSPHINX_LEGENDARY, &ANDROSPHINX_TEMPLATE),
            (SPHINX_OF_LORE_LEGENDARY, &SPHINX_OF_LORE_TEMPLATE),
            (DEATH_KNIGHT_LEGENDARY, &DEATH_KNIGHT_TEMPLATE),
        ]
    }

    fn arena() -> EncounterInstance {
        let tp = TerrainGenParams {
            width: 40,
            height: 40,
            branch_depth: 0,
            branch_prob: 0.0,
        };
        let ap = ActorGenParams {
            cr_target: 0.0,
            n_teams: 0,
            pc_template: None,
            start_team: 0,
        };
        EncounterInstance::from_params(&tp, &ap, Some(11)).unwrap()
    }

    /// Every option resolves against a populated board and against an
    /// empty one.
    ///
    /// The empty board is the half worth having, for the reason the
    /// lair suite gives: the dispatcher fires at the end of a turn,
    /// which is a moment that exists after the round that killed the
    /// last intruder and before the encounter notices it is over. An
    /// option that unwraps its victim takes the process down there.
    #[test]
    fn every_legendary_option_resolves_with_and_without_anybody_to_catch() {
        for (repertoire, template) in repertoires() {
            assert!(
                !repertoire.is_empty(),
                "{} has an empty repertoire",
                template.name
            );
            for entry in repertoire {
                for populated in [true, false] {
                    let mut e = arena();
                    let boss = e
                        .instantiate_creature(template, Coordinate::new(6, 6), 0, 0)
                        .unwrap();
                    if populated {
                        for (i, spot) in [(8, 6), (9, 7), (20, 20)].into_iter().enumerate() {
                            let _ = e.instantiate_creature(
                                &GOBLIN_TEMPLATE,
                                Coordinate::new(spot.0, spot.1),
                                1,
                                i,
                            );
                        }
                    }
                    (entry.fire)(&mut e, boss);
                }
            }
        }
    }

    /// A legendary action only ever catches the actor's enemies. Half
    /// of these options are bursts centred on the creature itself, and
    /// a dragon that knocks its own hobgoblins prone with its wings is
    /// the bug this pins.
    #[test]
    fn a_legendary_action_never_catches_its_own_side() {
        for (repertoire, template) in repertoires() {
            for entry in repertoire {
                let mut e = arena();
                let boss = e
                    .instantiate_creature(template, Coordinate::new(6, 6), 0, 0)
                    .unwrap();
                let ally = e
                    .instantiate_creature(&GOBLIN_TEMPLATE, Coordinate::new(10, 6), 0, 0)
                    .unwrap();
                let _ = e.instantiate_creature(&GOBLIN_TEMPLATE, Coordinate::new(11, 7), 1, 1);
                let boss_hp = e.actors[&boss].hitpoints();
                let ally_hp = e.actors[&ally].hitpoints();
                let boss_conditions = e.actors[&boss].conditions().len();
                let ally_conditions = e.actors[&ally].conditions().len();
                (entry.fire)(&mut e, boss);
                assert!(
                    e.actors[&boss].hitpoints() >= boss_hp,
                    "{}: {} hurt itself",
                    template.name,
                    entry.name
                );
                assert_eq!(
                    e.actors[&boss].conditions().len(),
                    boss_conditions,
                    "{}: {} landed a condition on itself",
                    template.name,
                    entry.name
                );
                assert_eq!(
                    e.actors[&ally].hitpoints(),
                    ally_hp,
                    "{}: {} hurt its own ally",
                    template.name,
                    entry.name
                );
                assert_eq!(
                    e.actors[&ally].conditions().len(),
                    ally_conditions,
                    "{}: {} landed a condition on its own ally",
                    template.name,
                    entry.name
                );
            }
        }
    }

    /// The two questions `SwingKind` answers are independent, and the
    /// table says so. Pinned because the pair is easy to collapse into
    /// one bool by somebody who notices that two of the three variants
    /// agree on either question taken alone.
    #[test]
    fn a_swing_kind_answers_melee_and_spell_separately() {
        assert!(SwingKind::Melee.is_melee());
        assert!(!SwingKind::Melee.is_spell());
        assert!(SwingKind::MeleeSpell.is_melee());
        assert!(SwingKind::MeleeSpell.is_spell());
        assert!(!SwingKind::RangedSpell.is_melee());
        assert!(SwingKind::RangedSpell.is_spell());
    }

    /// A ray fired across the room at a creature lying on the floor
    /// rolls at *disadvantage*, not advantage.
    ///
    /// 5e's prone clause is inverted between the two lanes — "attack
    /// rolls against the creature have advantage if the attacker is
    /// within 5 feet, disadvantage otherwise" — so a ranged option
    /// resolved as a melee swing is not a cosmetic mislabel: it is an
    /// eye ray that gets *better* the more of the party is on the
    /// floor. That is exactly what the beholder's ray did while every
    /// legendary swing shared one hardcoded `is_melee: true`.
    #[test]
    fn a_ranged_legendary_option_is_taxed_by_range_rather_than_rewarded_by_it() {
        use crate::actors::creatures::beholders::BEHOLDER_TEMPLATE;
        use crate::actors::creatures::goblins::GOBLIN_TEMPLATE;
        use crate::conditions::ConditionTimer;

        let mut e = arena();
        let beholder = e
            .instantiate_creature(&BEHOLDER_TEMPLATE, Coordinate::new(6, 6), 0, 0)
            .unwrap();
        let goblin = e
            .instantiate_creature(&GOBLIN_TEMPLATE, Coordinate::new(24, 6), 1, 1)
            .unwrap();
        e.actors
            .get_mut(&goblin)
            .unwrap()
            .add_condition(Condition::Prone, ConditionTimer::Permanent);

        let entry = &BEHOLDER_LEGENDARY[0];
        let before = e.messages().len();
        (entry.fire)(&mut e, beholder);
        let log = e.messages()[before..].join("\n");
        assert!(
            log.contains("(dis)"),
            "a ray at a prone target across the room should be at \
             disadvantage:\n{}",
            log
        );
        assert!(
            !log.contains("(adv)"),
            "and must not be rewarded for the target being down:\n{}",
            log
        );
    }

    /// Every repertoire written here is attached to the creature it was
    /// written for, and every creature that declares a legendary budget
    /// has one. The second half is the assertion this module exists to
    /// make true: a budget with no repertoire behind it is exactly the
    /// state eleven stat blocks were in before it.
    #[test]
    fn every_legendary_budget_has_a_repertoire_behind_it() {
        for (repertoire, template) in repertoires() {
            let attached: Vec<&str> =
                template.legendary_actions.iter().map(|a| a.name).collect();
            let written: Vec<&str> = repertoire.iter().map(|a| a.name).collect();
            assert_eq!(
                attached, written,
                "{} does not carry the repertoire written for it",
                template.name
            );
            assert!(
                template.legendary_actions_per_round > 0,
                "{} carries options it can never afford",
                template.name
            );
        }
        for (_, template) in repertoires() {
            assert!(
                !template.legendary_actions.is_empty(),
                "{} declares a legendary budget and has nothing to spend it on",
                template.name
            );
        }
    }

    /// No option is free, and none of them costs more than the budget
    /// of the cheapest creature that carries it. A `0`-cost option is a
    /// boss acting infinitely often; an unaffordable one is a line of
    /// the stat block nobody will ever see.
    #[test]
    fn every_option_is_priced_within_the_budget_that_pays_for_it() {
        for (repertoire, template) in repertoires() {
            for entry in repertoire {
                assert!(entry.cost >= 1, "{}: {} is free", template.name, entry.name);
                assert!(
                    entry.cost <= template.legendary_actions_per_round,
                    "{}: {} costs {} of a {}-point budget",
                    template.name,
                    entry.name,
                    entry.cost,
                    template.legendary_actions_per_round
                );
            }
        }
    }
}
