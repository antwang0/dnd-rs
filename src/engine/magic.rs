//! 5e's magical / nonmagical axis for attacks — the one fact a stat
//! block asks about a weapon more often than any other.
//!
//! "Resistance to bludgeoning, piercing, and slashing damage from
//! **nonmagical** attacks" is the most-written defensive clause in the
//! Monster Manual: forty stat blocks on this roster carry it, from the
//! CR-1 specter to the tarrasque. Until this module existed the engine
//! had nowhere to put the qualifier, so the clause shipped unqualified
//! — every one of those forty halved a +2 longsword exactly as it
//! halved a club, and the whole magic-weapon half of 5e's loot economy
//! bought nothing but a flat `+N`.
//!
//! The rule is deliberately narrow, and the narrowness is the design:
//!
//!   - It is about the **attack**, not about the damage. A wraith's
//!     resistance answers the swing; it has nothing to say about the
//!     bludgeoning a failed save against a collapsing ceiling deals,
//!     because that damage did not come from an attack. This is why the
//!     qualifier is read at the two attack chokepoints rather than at
//!     `DealDamage::apply`, which sees every damage instance in the
//!     engine and cannot tell which of them were attacks.
//!
//!   - It is about the **source of the attack**, not the target's
//!     senses or the damage type. Every spell attack is magical
//!     regardless of what it deals; a mundane arrow is not, regardless
//!     of who fired it.
//!
//! What follows is the roster of ways an attack in this engine can be
//! magical. It is a cohort table rather than a chain of `||` for the
//! usual reason — a new magic-weapon spell, item or monster trait
//! should land as one row, and the log line that explains the
//! unresisted swing should name the row that granted it.
//!
//! One clause on the roster is narrower than "nonmagical", and it is
//! why the chokepoint asks `resistance_bypass` rather than
//! `attack_is_magical`: the five lycanthropes write "from nonmagical
//! attacks **that aren't silvered**", so a 100 gp coating answers them
//! and nothing else. Which exemption applies is a property of the
//! *target's* clause, not of the attacker's weapon.

use crate::actions::class_features::{KI_EMPOWERED_STRIKES_TAG, MAGICAL_ATTACKS_TAG};
use crate::actors::actor_template::ActorInstance;
use crate::conditions::Condition;
use crate::engine::encounter::EncounterInstance;

/// One way an attack can count as magical.
struct MagicalAttackSource {
    /// Named in the log when this row is what let the swing through.
    /// Lowercase, the RAW feature or item name.
    label: &'static str,
    applies: fn(&ActorInstance) -> bool,
}

/// Every way a *weapon* attack in this engine becomes magical.
///
/// Spell attacks are not on the table: they are magical by definition
/// and are answered before the walk (see `attack_is_magical`).
///
/// The rows split into three lanes, and it is worth seeing that they
/// are three rather than one:
///
///   1. **The creature's own nature** — the monster tag, and the monk's
///      Ki-Empowered Strikes. Nothing can be taken away or dispelled.
///   2. **The weapon in its hand** — the `+1` / `+2` / `+3` loot tier.
///      Read off the inventory, so losing the item loses the property.
///      The Silvered Weapon is deliberately *not* here: silver is not
///      magic, answers only the lycanthrope clause, and rides
///      `resistance_bypass` instead.
///   3. **A spell on the weapon** — Magic Weapon, Elemental Weapon,
///      Holy Weapon, Shillelagh. Every one of these RAW says in so many
///      words that the weapon "becomes a magic weapon", and every one
///      of them is concentration-bound, so a broken concentration takes
///      the property back with the bonus.
///
/// Note what is deliberately *not* here: Divine Smite, Sneak Attack,
/// Hunter's Mark and the rest of the on-hit rider cohort. They add
/// damage to a swing; RAW they do not make the weapon magical, and the
/// radiant/necrotic damage they add is not physical damage in the first
/// place, so the qualified rows never touch it either way.
const MAGICAL_ATTACK_SOURCES: &[MagicalAttackSource] = &[
    MagicalAttackSource {
        label: "magical attacks",
        applies: |a| a.has_passive_feature(MAGICAL_ATTACKS_TAG),
    },
    MagicalAttackSource {
        label: "ki-empowered strikes",
        applies: |a| a.has_passive_feature(KI_EMPOWERED_STRIKES_TAG),
    },
    MagicalAttackSource {
        label: "a magic weapon",
        applies: |a| a.wields_enchanted_weapon(),
    },
    MagicalAttackSource {
        label: "the magic weapon spell",
        applies: |a| a.has_condition(Condition::WeaponEnchanted),
    },
    MagicalAttackSource {
        label: "elemental weapon",
        applies: |a| a.has_condition(Condition::ElementallyWeaponed),
    },
    MagicalAttackSource {
        label: "holy weapon",
        applies: |a| a.has_condition(Condition::HolyWeaponed),
    },
    MagicalAttackSource {
        label: "shillelagh",
        applies: |a| a.has_condition(Condition::Shillelaghed),
    },
];

/// Whether the swing `attacker_id` is making counts as a magical attack
/// for the purpose of overcoming resistance and immunity to nonmagical
/// attacks.
///
/// `is_spell` short-circuits to `true`: a spell attack roll is magical
/// whatever it deals and whoever makes it, and no row below could ever
/// take that back.
///
/// Returns the label of the row that answered, so the chokepoint can
/// say *why* a wraith failed to halve the blow. An unknown attacker
/// answers `None` — fails closed, which here means "mundane", which is
/// the right way for a lookup failure to land: the resistance applies
/// and the monster is no worse off than before this module existed.
pub fn magical_attack_source(
    encounter: &EncounterInstance,
    attacker_id: usize,
    is_spell: bool,
) -> Option<&'static str> {
    if is_spell {
        return Some("spell attack");
    }
    let attacker = encounter.actors.get(&attacker_id)?;
    MAGICAL_ATTACK_SOURCES
        .iter()
        .find(|row| (row.applies)(attacker))
        .map(|row| row.label)
}

/// The boolean half of `magical_attack_source`, for callers that only
/// need the verdict.
pub fn attack_is_magical(
    encounter: &EncounterInstance,
    attacker_id: usize,
    is_spell: bool,
) -> bool {
    magical_attack_source(encounter, attacker_id, is_spell).is_some()
}

/// Whether this swing gets through `target_id`'s **source-qualified**
/// damage modifiers, and what got it through.
///
/// The question the chokepoint actually asks, and deliberately not the
/// same question as `attack_is_magical`. Most of the roster writes
/// "from nonmagical attacks" and only magic answers it; the five
/// lycanthropes write "from nonmagical attacks **that aren't
/// silvered**", and a 100 gp coating answers that one too. Asking "is
/// the attack magical" would have made the silvered blade useless
/// against the one family of creatures it exists for.
///
/// Silver is checked second because it answers strictly less: a monk
/// with a silvered sword in their pack is through on their fists, and
/// the log should say so.
pub fn resistance_bypass(
    encounter: &EncounterInstance,
    attacker_id: usize,
    target_id: usize,
    is_spell: bool,
) -> Option<&'static str> {
    if let Some(label) = magical_attack_source(encounter, attacker_id, is_spell) {
        return Some(label);
    }
    let silver_helps = encounter
        .actors
        .get(&target_id)
        .is_some_and(|t| t.silver_overcomes_physical_resistance());
    let silvered = encounter
        .actors
        .get(&attacker_id)
        .is_some_and(|a| a.wields_silvered_weapon());
    (silver_helps && silvered).then_some("a silvered weapon")
}

/// Every stat block on the roster whose weapon attacks are magical, as
/// a written-down list.
///
/// A list rather than a predicate because there is nothing to derive it
/// from. RAW's **Magic Weapons** / **Angelic Weapons** trait is not
/// implied by creature type, CR, or anything else on the sheet: a
/// balor has it and a nalfeshnee of the same type and nearly the same
/// CR does not; an iron golem has it and a flesh golem does not; a
/// couatl has it at CR 4 and a pit fiend at CR 20. It is a fact about
/// the Monster Manual, so the Monster Manual's answer is what gets
/// stored.
///
/// Enforced by the sweep below in **both** directions, which is what
/// earns the list its keep. `MAGICAL_ATTACKS_TAG` is one short line
/// inside a `features` set that also holds Devil's Sight and half a
/// dozen other one-liners, and it is exactly the sort of line that
/// gets copy-pasted onto the next template down the file. A nalfeshnee
/// that quietly acquires it stops being answerable by a wraith's own
/// claws, and nothing about that shows up as a crash — only as a fight
/// that plays slightly wrong.
/// Holds the templates themselves rather than their names, so the list
/// stays honest about who carries the trait even if a stat block later
/// drops out of the encounter pool — the roster is what the trait
/// belongs to, not the generator.
#[cfg(test)]
fn templates_with_magical_weapon_attacks() -> Vec<&'static crate::actors::actor_template::CreatureTemplate>
{
    use crate::actors::creatures::*;
    vec![
        &androsphinxes::ANDROSPHINX_TEMPLATE,
        &balors::BALOR_TEMPLATE,
        &barbed_devils::BARBED_DEVIL_TEMPLATE,
        &bearded_devils::BEARDED_DEVIL_TEMPLATE,
        &bone_devils::BONE_DEVIL_TEMPLATE,
        &chain_devils::CHAIN_DEVIL_TEMPLATE,
        &couatls::COUATL_TEMPLATE,
        &devas::DEVA_TEMPLATE,
        &erinyes::ERINYES_TEMPLATE,
        &clay_golems::CLAY_GOLEM_TEMPLATE,
        &horned_devils::HORNED_DEVIL_TEMPLATE,
        // The ice devil and the planetar join by cohort rather than by
        // stat block, and it is worth saying so out loud: SRD 5.2 prints
        // no Magic Weapons trait on *either* of them — nor, for that
        // matter, on the bone devil, the deva, the pit fiend or the
        // solar already above them. The 2024 revision folded the trait
        // away and this roster kept the 2014 answer, which is a defensible
        // choice made once and a mess if it is made per stat block: an
        // ice devil whose sword a wraith halves, standing beside a bone
        // devil's whose it doesn't, is the only reading that is wrong
        // under both editions.
        &ice_devils::ICE_DEVIL_TEMPLATE,
        &planetars::PLANETAR_TEMPLATE,
        &iron_golems::IRON_GOLEM_TEMPLATE,
        &mariliths::MARILITH_TEMPLATE,
        &pit_fiends::PIT_FIEND_TEMPLATE,
        &solars::SOLAR_TEMPLATE,
        &stone_golems::STONE_GOLEM_TEMPLATE,
        &unicorns::UNICORN_TEMPLATE,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::creatures::goblins::GOBLIN_TEMPLATE;
    use crate::conditions::ConditionTimer;
    use crate::engine::actor_gen::ActorGenParams;
    use crate::engine::encounter::EncounterInstance;
    use crate::engine::terrain_gen::TerrainGenParams;
    use crate::engine::types::{Coordinate, DamageModifier, DamageType};

    fn arena() -> EncounterInstance {
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

    /// The forty stat blocks that carry the clause reach the engine
    /// with it in the qualified table and *not* in the unqualified one.
    ///
    /// Both halves matter. A B/P/S row that stayed unqualified would
    /// halve a magic sword exactly as before, silently reverting the
    /// feature for that monster; a qualified row shadowed by an
    /// unqualified one of the same type would be dead code that reads
    /// like a rule.
    #[test]
    fn the_bps_triplet_is_qualified_and_only_qualified() {
        for t in EncounterInstance::template_pool() {
            for dt in [
                DamageType::Bludgeoning,
                DamageType::Piercing,
                DamageType::Slashing,
            ] {
                let Some(qualified) = t.nonmagical_damage_modifiers.get(&dt) else {
                    continue;
                };
                assert_eq!(
                    *qualified,
                    DamageModifier::Resistance,
                    "{} qualifies {:?} with something other than resistance",
                    t.name,
                    dt
                );
                assert!(
                    !t.damage_modifiers.contains_key(&dt),
                    "{} carries both a qualified and an unqualified {:?} row; the \
                     unqualified one shadows the other and the swing is halved either way",
                    t.name,
                    dt
                );
            }
        }
    }

    /// A goblin with nothing magical on it swings mundanely, and the
    /// same goblin with a `+1 Weapon` in its pack does not.
    #[test]
    fn a_plus_one_weapon_makes_a_swing_magical() {
        let mut e = arena();
        let id = e
            .instantiate_creature(&GOBLIN_TEMPLATE, Coordinate::new(3, 3), 0, 0)
            .unwrap();
        assert!(!attack_is_magical(&e, id, false));
        e.actors
            .get_mut(&id)
            .unwrap()
            .pickup_item(&crate::items::item_template::WEAPON_PLUS_ONE);
        assert_eq!(magical_attack_source(&e, id, false), Some("a magic weapon"));
    }

    /// Shillelagh magics the club it is cast on, and dropping the
    /// condition takes the property back with it.
    #[test]
    fn a_weapon_spell_magics_the_swing_for_as_long_as_it_lasts() {
        let mut e = arena();
        let id = e
            .instantiate_creature(&GOBLIN_TEMPLATE, Coordinate::new(3, 3), 0, 0)
            .unwrap();
        let goblin = e.actors.get_mut(&id).unwrap();
        goblin.add_condition(Condition::Shillelaghed, ConditionTimer::Permanent);
        assert_eq!(magical_attack_source(&e, id, false), Some("shillelagh"));
        e.actors
            .get_mut(&id)
            .unwrap()
            .remove_condition(Condition::Shillelaghed);
        assert!(!attack_is_magical(&e, id, false));
    }

    /// A spell attack is magical with nothing equipped and nothing cast
    /// on it — the short-circuit no row can override.
    #[test]
    fn a_spell_attack_is_magical_on_its_own() {
        let mut e = arena();
        let id = e
            .instantiate_creature(&GOBLIN_TEMPLATE, Coordinate::new(3, 3), 0, 0)
            .unwrap();
        assert_eq!(magical_attack_source(&e, id, true), Some("spell attack"));
    }

    /// An attacker who is not on the board answers "mundane" rather
    /// than panicking or defaulting to magical.
    #[test]
    fn an_absent_attacker_swings_mundanely() {
        let e = arena();
        assert!(!attack_is_magical(&e, 9999, false));
    }

    /// `MAGICAL_ATTACKS_TAG` is carried by exactly the stat blocks on
    /// the list — every one of them, and nothing else anywhere the
    /// engine can reach. See the list for why both directions matter.
    #[test]
    fn the_magical_attacks_tag_is_carried_by_exactly_the_listed_stat_blocks() {
        let expected = templates_with_magical_weapon_attacks();
        for t in &expected {
            assert!(
                t.features.contains(MAGICAL_ATTACKS_TAG),
                "{} is listed as having magical weapon attacks and does not carry the tag",
                t.name
            );
        }
        let listed: Vec<&str> = expected.iter().map(|t| t.name).collect();
        let everything = EncounterInstance::template_pool().into_iter().chain(
            crate::actors::creatures::pc_template_families()
                .into_iter()
                .flat_map(|(_, ts)| ts),
        );
        for t in everything {
            if !t.features.contains(MAGICAL_ATTACKS_TAG) {
                continue;
            }
            assert!(
                listed.contains(&t.name),
                "{} carries the magical-attacks tag but is not on the list",
                t.name
            );
        }
    }

    /// A creature that already halves the swing is scored as a worse
    /// target for it than one that doesn't, and the same creature stops
    /// being a worse target the moment the swinger picks up a magic
    /// weapon.
    ///
    /// The AI's matchup lane read the unqualified table only, which was
    /// the whole answer while the B/P/S triplet lived there. Moving the
    /// triplet to the qualified table would have silently blinded it —
    /// the code would still compile, every test above would still pass,
    /// and a wraith would simply stop looking any different from a
    /// goblin to a creature holding a sword.
    #[test]
    fn the_ai_sees_a_resistance_only_when_its_swing_is_answered_by_it() {
        use crate::actors::creatures::wraiths::WRAITH_TEMPLATE;
        use crate::ai::simple::matchup_penalty_against;

        let mut e = arena();
        let wraith = e
            .instantiate_creature(&WRAITH_TEMPLATE, Coordinate::new(3, 3), 0, 0)
            .unwrap();
        let goblin = e
            .instantiate_creature(&GOBLIN_TEMPLATE, Coordinate::new(4, 3), 1, 0)
            .unwrap();
        let slashing = &[DamageType::Slashing][..];
        assert_eq!(matchup_penalty_against(&e, goblin, wraith, slashing), 2);
        e.actors
            .get_mut(&goblin)
            .unwrap()
            .pickup_item(&crate::items::item_template::WEAPON_PLUS_ONE);
        assert_eq!(matchup_penalty_against(&e, goblin, wraith, slashing), 1);
    }

    /// Silver gets through a lycanthrope and through nothing else.
    ///
    /// Both halves are the feature. RAW's silvering is a 100 gp
    /// purchase whose entire value is one family of monsters, and a
    /// silvered blade that also answered a wraith would be a magic
    /// weapon that cost a hundredth as much.
    #[test]
    fn silver_answers_the_werewolf_and_not_the_wraith() {
        use crate::actors::creatures::werewolves::WEREWOLF_TEMPLATE;
        use crate::actors::creatures::wraiths::WRAITH_TEMPLATE;
        use crate::items::item_template::SILVERED_WEAPON;

        let mut e = arena();
        let goblin = e
            .instantiate_creature(&GOBLIN_TEMPLATE, Coordinate::new(3, 3), 0, 0)
            .unwrap();
        let werewolf = e
            .instantiate_creature(&WEREWOLF_TEMPLATE, Coordinate::new(6, 3), 1, 0)
            .unwrap();
        let wraith = e
            .instantiate_creature(&WRAITH_TEMPLATE, Coordinate::new(9, 3), 1, 0)
            .unwrap();

        // Bare-handed, neither is answerable.
        assert_eq!(resistance_bypass(&e, goblin, werewolf, false), None);
        assert_eq!(resistance_bypass(&e, goblin, wraith, false), None);

        e.actors
            .get_mut(&goblin)
            .unwrap()
            .pickup_item(&SILVERED_WEAPON);
        assert_eq!(
            resistance_bypass(&e, goblin, werewolf, false),
            Some("a silvered weapon")
        );
        assert_eq!(
            resistance_bypass(&e, goblin, wraith, false),
            None,
            "silver is not magic and a wraith's clause does not name it"
        );

        // The silvered weapon does not make the swing magical — the two
        // questions stay separate, which is what keeps the wraith
        // answering no above.
        assert!(!attack_is_magical(&e, goblin, false));
    }

    /// Exactly the five lycanthrope stat blocks name silver in their
    /// clause. Swept in both directions for the same reason every
    /// one-line flag here is: a wraith that quietly acquired it would
    /// make the cheapest item in the loot pool answer the whole
    /// incorporeal-undead family, and nothing about that reads as a
    /// bug from any other angle.
    #[test]
    fn the_silver_exemption_belongs_to_exactly_the_lycanthropes() {
        use crate::actors::creatures::*;
        let expected = [
            &werebears::WEREBEAR_TEMPLATE,
            &wereboars::WEREBOAR_TEMPLATE,
            &wererats::WERERAT_TEMPLATE,
            &weretigers::WERETIGER_TEMPLATE,
            &werewolves::WEREWOLF_TEMPLATE,
        ];
        for t in expected {
            assert!(
                t.silver_overcomes_physical_resistance,
                "{} is a lycanthrope and does not name silver",
                t.name
            );
        }
        let listed: Vec<&str> = expected.iter().map(|t| t.name).collect();
        let everything = EncounterInstance::template_pool().into_iter().chain(
            crate::actors::creatures::pc_template_families()
                .into_iter()
                .flat_map(|(_, ts)| ts),
        );
        for t in everything {
            if !t.silver_overcomes_physical_resistance {
                continue;
            }
            assert!(
                listed.contains(&t.name),
                "{} lets silver through and is not a lycanthrope",
                t.name
            );
        }
    }

    /// 5e's single most common defensive clause is *"resistance to
    /// bludgeoning, piercing, and slashing damage from **nonmagical
    /// attacks**"*, and the qualifier is the whole of what makes a +1
    /// sword worth carrying. A stat block that writes the triplet into
    /// the *unqualified* table has quietly deleted that: the magic
    /// weapon, the Divine Smite, the Ki-Empowered fist and the silvered
    /// blade all stop being answers, and nothing about it reads as a bug
    /// from any other angle — the creature simply resists everything a
    /// little, forever, and every test of it still passes.
    ///
    /// It is a mistake with a history here. The elemental chassis
    /// carried the unqualified triplet across seventeen stat blocks
    /// whose own docstrings all said "non-magical physical", the deva
    /// carried it with a comment apologising for the collapse, and both
    /// were written back when the engine genuinely had no magic axis to
    /// write against. The concessions outlived the gap. This sweep is so
    /// that the next one cannot.
    ///
    /// The allowlist is short and none of it is about magic at all.
    /// A swarm resists physical damage because it is a cloud of
    /// individually-tiny things and a blade passes between them; RAW
    /// writes that clause *without* the qualifier, and a magic blade
    /// passes between them just as uselessly. The treant is the same
    /// shape for a different reason — it is a tree, and enchanting the
    /// axe does not make the trunk thinner — and RAW writes its
    /// "bludgeoning, piercing" unqualified too. Note that the treant's
    /// pair is only two types deep: the sweep is per-type rather than
    /// all-or-nothing precisely so that a stat block cannot hide a
    /// blanket resistance by carrying only two thirds of the triplet.
    #[test]
    fn a_physical_resistance_that_magic_cannot_answer_is_a_swarm_or_a_bug() {
        use crate::engine::types::{DamageModifier, DamageType};

        const PHYSICAL: [DamageType; 3] = [
            DamageType::Bludgeoning,
            DamageType::Piercing,
            DamageType::Slashing,
        ];
        // RAW's own unqualified physical resistance, and the only one.
        let unqualified_in_raw = [
            // A cloud of individually-tiny things: the blade passes
            // between them, and enchanting it does not make it wider.
            "Swarm of Bats",
            "Swarm of Insects",
            "Swarm of Piranhas",
            "Swarm of Venomous Snakes",
            // Wood. Enchanting the axe does not thin the trunk.
            "Treant",
            "Awakened Tree",
            "Awakened Shrub",
            // A body with no soft parts to run through — the 2024
            // skeleton's Piercing row and the flameskull's.
            "Skeleton",
            "Flameskull",
            // An ooze: cutting it in half produces two of it. RAW gives
            // both of these Slashing and neither of them a qualifier.
            "Black Pudding",
            "Ochre Jelly",
        ];

        // Collected rather than asserted one at a time: the first run of
        // this sweep had eight distinct answers in it, and a fail-fast
        // version would have surfaced them one `cargo test` at a time.
        let mut offenders: Vec<String> = Vec::new();
        let everything = EncounterInstance::template_pool().into_iter().chain(
            crate::actors::creatures::pc_template_families()
                .into_iter()
                .flat_map(|(_, ts)| ts),
        );
        for t in everything {
            let unqualified: Vec<DamageType> = PHYSICAL
                .into_iter()
                .filter(|dt| {
                    matches!(
                        t.damage_modifiers.get(dt),
                        Some(DamageModifier::Resistance | DamageModifier::Immunity)
                    )
                })
                .collect();
            if unqualified.is_empty() || unqualified_in_raw.contains(&t.name) {
                continue;
            }
            offenders.push(format!("{} ({:?})", t.name, unqualified));
        }
        assert!(
            offenders.is_empty(),
            "these stat blocks resist physical damage from every source, magical or not — if \
             that is RAW the name belongs on the allowlist here, and if it is not the rows \
             belong in nonmagical_damage_modifiers: {}",
            offenders.join(", ")
        );
    }

    /// The other direction, and the reason the pair is worth having:
    /// a template must not carry the *same* physical type in both
    /// tables. `nonmagical_damage_modifier` resolves that collision by
    /// returning `None` — the unqualified row wins and the qualified one
    /// silently does nothing — so a stat block written that way would
    /// have a resistance clause in its source that the engine never
    /// reads, which is worse than either honest answer.
    #[test]
    fn no_stat_block_writes_the_same_type_into_both_damage_tables() {
        let everything = EncounterInstance::template_pool().into_iter().chain(
            crate::actors::creatures::pc_template_families()
                .into_iter()
                .flat_map(|(_, ts)| ts),
        );
        for t in everything {
            for dt in t.nonmagical_damage_modifiers.keys() {
                assert!(
                    !t.damage_modifiers.contains_key(dt),
                    "{} carries {:?} in both tables; the qualified row is dead code",
                    t.name,
                    dt
                );
            }
        }
    }

    /// The monk's answer to the clause reaches an instantiated monk.
    /// Worth pinning separately from the tag itself: Ki-Empowered
    /// Strikes is the *only* way a character with no loot and no
    /// spellcaster in the party can hurt a wraith at full rate, so a
    /// subclass that stopped inheriting it would lose something no
    /// other row could give back.
    #[test]
    fn every_monk_build_punches_through_mundane_resistance() {
        use crate::actors::creatures::pc_template_families;
        let monks = pc_template_families()
            .into_iter()
            .find(|(family, _)| *family == "monk")
            .expect("the monk family is registered")
            .1;
        for t in monks {
            let mut e = arena();
            let id = e
                .instantiate_creature(t, Coordinate::new(3, 3), 0, 0)
                .unwrap();
            assert_eq!(
                magical_attack_source(&e, id, false),
                Some("ki-empowered strikes"),
                "{}",
                t.name
            );
        }
    }
}
