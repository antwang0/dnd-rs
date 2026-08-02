//! 5e **lair actions** — the effects a place takes while its resident
//! is alive inside it.
//!
//! Everything else in the engine that changes the board is an `Action`,
//! because somebody chose it: the trait is built around a caster, a
//! resource cost, a targeting schema, and a validation pass over
//! arguments a player or the AI supplied. A lair action has none of
//! those. Nobody pays for it, nobody aims it, and it fires on a
//! creature's behalf whether or not that creature could act — a
//! paralyzed dragon's cave still shakes. Modeling one as an `Action`
//! would mean an action with an empty cost, a schema that takes no
//! arguments, a validation that always passes, and a caster who isn't
//! casting; four lies to reuse a trait for its vocabulary.
//!
//! So a lair action is a name and a function. The function reads the
//! board and picks its own victims, which is exactly what the rules
//! text does ("each creature in the lair", "one creature the dragon can
//! see"), and it returns nothing — like the reaction dispatcher, it
//! resolves eagerly rather than through the side-effect stack, because
//! there is no turn for it to be part of.
//!
//! Attaching a lair to a creature is one field on its template. The
//! dispatcher (`EncounterInstance::dispatch_lair_actions`) does the
//! rest: it fires once per round, for one resident, choosing an entry
//! the lair didn't just use.

use crate::actions::action_template::{
    resolve_enemy_burst_save_damage, resolve_burst_save_condition,
};
use crate::conditions::{Condition, ConditionTimer};
use crate::engine::dice::Dice;
use crate::engine::encounter::EncounterInstance;
use crate::engine::saves::SaveDamagePolicy;
use crate::engine::side_effects::{ApplicableSideEffect, DealDamage, Heal};
use crate::engine::types::{AbilityScoreType, DamageType};

/// One entry in a creature's lair repertoire.
///
/// `fire` takes the encounter and the id of the lair's resident. It is
/// handed the resident rather than a caster because the resident is
/// what the effect is *about* — whose enemies to catch, where the
/// centre of the room is, whose save DC the lair borrows — not who is
/// spending anything.
pub struct LairAction {
    /// Shown in the log when this one fires. Written as the lair's
    /// doing, not the creature's.
    pub name: &'static str,
    /// Resolve the effect. Applies its own side-effects directly.
    pub fire: fn(&mut EncounterInstance, usize),
}

/// The DC a lair rolls against. RAW prints a fixed number per lair
/// (DC 15 for a dragon's tremors, DC 18 for a lich's tether); we take
/// the resident's own spell save DC, floored, so a lair scales with the
/// thing that lives in it instead of with a table of literals. The
/// floor is what keeps a lair dangerous when its resident is a martial
/// creature with no casting ability to derive a DC from.
fn lair_dc(encounter: &EncounterInstance, resident_id: usize) -> i32 {
    encounter
        .actors
        .get(&resident_id)
        .map(|a| {
            a.best_spell_save_dc([
                AbilityScoreType::Charisma,
                AbilityScoreType::Intelligence,
                AbilityScoreType::Wisdom,
                AbilityScoreType::Constitution,
            ])
        })
        .unwrap_or(LAIR_DC_FLOOR)
        .max(LAIR_DC_FLOOR)
}

/// Floor for `lair_dc`. Roughly the DC a CR-10 lair prints.
const LAIR_DC_FLOOR: i32 = 15;

/// Radius, in tiles, of a lair effect that catches "each creature in
/// the lair". The board is the lair, so this is deliberately larger
/// than any spell radius — 20 tiles is 100 ft, which covers a generated
/// map end to end.
const LAIR_WIDE: isize = 20;

/// Radius of a lair effect that erupts at a point. 4 tiles = 20 ft,
/// the radius RAW prints for a dragon's magma eruption.
const LAIR_ERUPTION: isize = 4;

/// Centre of a lair effect that erupts *somewhere*. RAW lets the
/// resident pick a point it can see; the lair picks the enemy cluster
/// instead — the tile of whichever hostile has the most company within
/// the eruption radius, so the effect lands where it does the most and
/// never on an empty corner. Falls back to the resident's own tile when
/// nothing hostile is on the board.
fn eruption_point(
    encounter: &EncounterInstance,
    resident_id: usize,
) -> Option<crate::engine::types::Coordinate> {
    let resident_loc = encounter.actors.get(&resident_id)?.location();
    let candidates = encounter.enemy_burst_targets(resident_id, resident_loc, LAIR_WIDE);
    candidates
        .iter()
        .map(|&id| {
            let loc = encounter.actors[&id].location();
            let company = encounter
                .enemy_burst_targets(resident_id, loc, LAIR_ERUPTION)
                .len();
            (company, id, loc)
        })
        // Most company wins; the lowest id breaks the tie so the choice
        // is stable across runs the way every other board sweep here is.
        .max_by_key(|(company, id, _)| (*company, std::cmp::Reverse(*id)))
        .map(|(_, _, loc)| loc)
        .or(Some(resident_loc))
}

/// Resolve a "each creature in the lair must save or take damage" body,
/// which several of the entries below share.
///
/// Centred on `eruption_point` rather than on the resident, which
/// matters for the tight-radius effects and costs the wide ones
/// nothing: an eruption that goes off under the dragon's own feet has
/// no intruders in it, and the wide ones cover the map from wherever
/// they start. Targets come from the engine's enemy-only burst filter,
/// which is the one call that says what every lair effect in the rules
/// says — the resident and its allies are not in it.
fn lair_burst_damage(
    encounter: &mut EncounterInstance,
    resident_id: usize,
    radius: isize,
    save: AbilityScoreType,
    dice: Dice,
    damage_type: DamageType,
    policy: SaveDamagePolicy,
) {
    let Some(center) = eruption_point(encounter, resident_id) else {
        return;
    };
    let dc = lair_dc(encounter, resident_id);
    let damage = encounter.roll(&dice);
    let effects = resolve_enemy_burst_save_damage(
        encounter,
        resident_id,
        center,
        radius,
        save,
        dc,
        damage,
        damage_type,
        policy,
    );
    for effect in effects {
        effect.apply(encounter);
    }
}

/// Resolve a "each creature in the lair must save or pick something up"
/// body — the condition half of the same shape, centred the same way
/// and for the same reason.
fn lair_burst_condition(
    encounter: &mut EncounterInstance,
    resident_id: usize,
    radius: isize,
    save: AbilityScoreType,
    condition: Condition,
    timer: ConditionTimer,
) {
    let Some(center) = eruption_point(encounter, resident_id) else {
        return;
    };
    let dc = lair_dc(encounter, resident_id);
    let effects = resolve_burst_save_condition(
        encounter, resident_id, center, radius, save, dc, condition, timer,
    );
    for effect in effects {
        effect.apply(encounter);
    }
}

/// **Dragon lair** (MM, chromatic dragon lairs). Three effects: the
/// floor erupts, the floor shakes, and the air turns against everyone
/// breathing it.
pub const DRAGON_LAIR: &[LairAction] = &[
    LairAction {
        name: "magma erupts from a fissure",
        fire: |e, id| {
            lair_burst_damage(
                e,
                id,
                LAIR_ERUPTION,
                AbilityScoreType::Dexterity,
                Dice::new(6, 6),
                DamageType::Fire,
                // RAW is "or take 6d6 fire damage" — a made save takes
                // nothing at all, which is what distinguishes an
                // eruption you can step out of from a breath weapon you
                // can only flinch from.
                SaveDamagePolicy::NoneOnSave,
            )
        },
    },
    LairAction {
        name: "a tremor shakes the lair",
        fire: |e, id| {
            lair_burst_condition(
                e,
                id,
                LAIR_WIDE,
                AbilityScoreType::Dexterity,
                Condition::Prone,
                ConditionTimer::Permanent,
            )
        },
    },
    LairAction {
        name: "volcanic gases form a choking cloud",
        fire: |e, id| {
            lair_burst_condition(
                e,
                id,
                LAIR_ERUPTION,
                AbilityScoreType::Constitution,
                Condition::Poisoned,
                ConditionTimer::Rounds(2),
            )
        },
    },
];

/// **Lich lair** (MM). The dead of the place answer to it: a tether of
/// negative energy that feeds the lich what it drains, and the shades
/// of everything that has died here.
pub const LICH_LAIR: &[LairAction] = &[
    LairAction {
        name: "a cord of negative energy tethers the living",
        fire: |e, id| {
            let dc = lair_dc(e, id);
            let Some(loc) = e.actors.get(&id).map(|a| a.location()) else {
                return;
            };
            // RAW targets one creature the lich can see within 30 ft.
            // The nearest hostile is that creature; ids break ties, the
            // way every other single-target sweep in the engine does.
            let Some(&victim) = e.enemy_burst_targets(id, loc, 6).first() else {
                return;
            };
            if e.roll_save(victim, AbilityScoreType::Constitution, dc).passed() {
                return;
            }
            let drained = e.roll(&Dice::new(3, 6));
            let victim_name = e.actor_name(victim);
            let lich_name = e.actor_name(id);
            e.log(format!(
                "  the cord bleeds {} of {} \u{2014} {} drinks it.",
                victim_name, drained, lich_name
            ));
            DealDamage {
                actor_id: victim,
                amount: drained,
                damage_type: DamageType::Necrotic,
            }
            .apply(e);
            Heal {
                actor_id: id,
                amount: drained,
            }
            .apply(e);
        },
    },
    LairAction {
        name: "the spirits of the lair's dead rise",
        fire: |e, id| {
            lair_burst_condition(
                e,
                id,
                LAIR_WIDE,
                AbilityScoreType::Wisdom,
                Condition::Frightened,
                ConditionTimer::Rounds(2),
            )
        },
    },
    LairAction {
        name: "the lair's wards knit a spent weave back together",
        fire: |e, id| {
            // RAW: "the lich rolls a d8 and regains a spell slot of that
            // level or lower". The d8 is rolled through the encounter's
            // seeded roller so a replay lands the same slot back.
            let rolled = e.roll(&Dice::new(1, 8));
            let name = e.actor_name(id);
            let restored = e.actors.get_mut(&id).and_then(|a| {
                a.spell_slot_manager
                    .restore_highest_expended_slot_up_to(rolled)
            });
            let Some(level) = restored else {
                e.log(format!("  the wards find nothing of {}'s left to mend.", name));
                return;
            };
            e.log(format!(
                "  the wards mend a level-{} weave for {}.",
                level, name
            ));
        },
    },
];

/// **Kraken lair** (MM). The water itself is the weapon: a squall that
/// throws everything off its footing, and lightning that walks the
/// surface.
pub const KRAKEN_LAIR: &[LairAction] = &[
    LairAction {
        name: "lightning walks the water",
        fire: |e, id| {
            lair_burst_damage(
                e,
                id,
                LAIR_ERUPTION,
                AbilityScoreType::Dexterity,
                Dice::new(4, 6),
                DamageType::Lightning,
                SaveDamagePolicy::HalfOnSave,
            )
        },
    },
    LairAction {
        name: "a riptide drags everything under",
        fire: |e, id| {
            lair_burst_condition(
                e,
                id,
                LAIR_WIDE,
                AbilityScoreType::Strength,
                Condition::Prone,
                ConditionTimer::Permanent,
            )
        },
    },
    LairAction {
        name: "a squall of freezing spray sweeps the lair",
        fire: |e, id| {
            lair_burst_damage(
                e,
                id,
                LAIR_WIDE,
                AbilityScoreType::Constitution,
                Dice::new(2, 6),
                DamageType::Cold,
                SaveDamagePolicy::HalfOnSave,
            )
        },
    },
];

/// **Beholder lair** (MM). The aberration's paranoia leaks into the
/// stone around it: walls that grasp, and eyes that open where nobody
/// is looking.
pub const BEHOLDER_LAIR: &[LairAction] = &[
    LairAction {
        name: "slimy walls grasp at intruders",
        fire: |e, id| {
            lair_burst_condition(
                e,
                id,
                LAIR_WIDE,
                AbilityScoreType::Strength,
                Condition::Restrained,
                ConditionTimer::Rounds(2),
            )
        },
    },
    LairAction {
        name: "an eye opens in the stone and stares",
        fire: |e, id| {
            lair_burst_condition(
                e,
                id,
                LAIR_ERUPTION,
                AbilityScoreType::Wisdom,
                Condition::Blinded,
                ConditionTimer::Rounds(2),
            )
        },
    },
    LairAction {
        name: "the lair's geometry lurches",
        fire: |e, id| {
            lair_burst_damage(
                e,
                id,
                LAIR_WIDE,
                AbilityScoreType::Dexterity,
                Dice::new(3, 6),
                DamageType::Psychic,
                SaveDamagePolicy::HalfOnSave,
            )
        },
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::creatures::beholders::BEHOLDER_TEMPLATE;
    use crate::actors::creatures::dragons::ADULT_RED_DRAGON_TEMPLATE;
    use crate::actors::creatures::goblins::GOBLIN_TEMPLATE;
    use crate::actors::creatures::krakens::KRAKEN_TEMPLATE;
    use crate::actors::creatures::liches::LICH_TEMPLATE;
    use crate::actors::actor_template::CreatureTemplate;
    use crate::engine::actor_gen::ActorGenParams;
    use crate::engine::terrain_gen::TerrainGenParams;
    use crate::engine::types::Coordinate;

    /// Every lair, paired with the creature it belongs to.
    ///
    /// The pairing is what the sweeps below need, and writing it down
    /// once is what keeps a new lair from being added with nothing
    /// exercising it: a table entry with no template attached fails the
    /// reachability sweep rather than sitting inert.
    fn lairs() -> Vec<(&'static [LairAction], &'static CreatureTemplate)> {
        vec![
            (DRAGON_LAIR, &*ADULT_RED_DRAGON_TEMPLATE),
            (LICH_LAIR, &*LICH_TEMPLATE),
            (KRAKEN_LAIR, &*KRAKEN_TEMPLATE),
            (BEHOLDER_LAIR, &*BEHOLDER_TEMPLATE),
        ]
    }

    fn arena() -> EncounterInstance {
        let tp = TerrainGenParams {
            width: 30,
            height: 30,
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

    /// Every entry in every lair resolves against a populated board
    /// without panicking, and against an empty one too.
    ///
    /// The empty board is the half worth having. A lair fires on a
    /// schedule nobody controls — after the round that killed the last
    /// intruder, in the tick before the encounter notices it is over —
    /// so "there is nobody to catch" is a state each entry genuinely
    /// reaches, and the ones that pick a victim off the board are one
    /// unwrapped `Option` away from taking the process down when they
    /// get there.
    #[test]
    fn every_lair_action_resolves_with_and_without_anybody_to_catch() {
        for (lair, template) in lairs() {
            assert!(!lair.is_empty(), "{} has an empty lair", template.name);
            for entry in lair {
                for populated in [true, false] {
                    let mut e = arena();
                    let resident = e
                        .instantiate_creature(template, Coordinate::new(3, 3), 0, 0)
                        .unwrap();
                    if populated {
                        for (i, spot) in
                            [(10, 10), (11, 10), (20, 20)].into_iter().enumerate()
                        {
                            let _ = e.instantiate_creature(
                                &GOBLIN_TEMPLATE,
                                Coordinate::new(spot.0, spot.1),
                                1,
                                i,
                            );
                        }
                    }
                    (entry.fire)(&mut e, resident);
                }
            }
        }
    }

    /// A lair only ever catches the resident's enemies. Every lair
    /// effect in the rules exempts the creature whose lair it is, and
    /// several of them would otherwise be strictly self-defeating — a
    /// dragon that knocks itself prone with its own tremor.
    #[test]
    fn a_lair_never_catches_its_own_resident() {
        for (lair, template) in lairs() {
            for entry in lair {
                let mut e = arena();
                let resident = e
                    .instantiate_creature(template, Coordinate::new(3, 3), 0, 0)
                    .unwrap();
                let ally = e
                    .instantiate_creature(&GOBLIN_TEMPLATE, Coordinate::new(4, 3), 0, 0)
                    .unwrap();
                let _ = e.instantiate_creature(&GOBLIN_TEMPLATE, Coordinate::new(5, 3), 1, 1);
                let resident_hp = e.actors[&resident].hitpoints();
                let ally_hp = e.actors[&ally].hitpoints();
                let resident_conditions = e.actors[&resident].conditions().len();
                let ally_conditions = e.actors[&ally].conditions().len();
                (entry.fire)(&mut e, resident);
                // The lich's tether heals its resident, so HP is allowed
                // to rise — it just must never fall.
                assert!(
                    e.actors[&resident].hitpoints() >= resident_hp,
                    "{}: {} hurt its own resident",
                    template.name,
                    entry.name
                );
                assert_eq!(
                    e.actors[&resident].conditions().len(),
                    resident_conditions,
                    "{}: {} landed a condition on its own resident",
                    template.name,
                    entry.name
                );
                assert_eq!(
                    e.actors[&ally].hitpoints(),
                    ally_hp,
                    "{}: {} hurt the resident's ally",
                    template.name,
                    entry.name
                );
                assert_eq!(
                    e.actors[&ally].conditions().len(),
                    ally_conditions,
                    "{}: {} landed a condition on the resident's ally",
                    template.name,
                    entry.name
                );
            }
        }
    }

    /// Every lair written here is attached to the creature it was
    /// written for. A table nothing points at is a table that never
    /// fires, and the failure is silent — the encounter simply plays
    /// out as if the place were an ordinary room.
    #[test]
    fn every_lair_is_reachable_from_its_creature() {
        for (lair, template) in lairs() {
            // Compared by content rather than by address: these tables
            // are `const`, so each use site gets its own copy and
            // pointer identity would say "different" about two views of
            // the same list.
            let attached: Vec<&str> =
                template.lair_actions.iter().map(|a| a.name).collect();
            let written: Vec<&str> = lair.iter().map(|a| a.name).collect();
            assert_eq!(
                attached, written,
                "{} does not carry the lair written for it",
                template.name
            );
        }
    }
}
