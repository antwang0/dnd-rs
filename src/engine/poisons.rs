//! **Poison** — SRD 5.2's *Poison* section, or the quarter of it that
//! happens inside a six-second round.
//!
//! The book sorts its sample poisons by how they get into you, and the
//! four types are four completely different pieces of machinery:
//!
//!   - **Injury.** *"Can be applied as a Bonus Action to a weapon, a
//!     piece of ammunition, or similar object. … A creature that takes
//!     Piercing or Slashing damage from an object coated with the
//!     poison is exposed to its effects."* A bonus action, a swing, a
//!     Constitution save. This module is those.
//!   - **Contact.** *"Smeared on an object and remains potent until it
//!     is touched or washed off."* The trap layer's shape without the
//!     trap layer's tile: the poison is on a doorknob, and the engine
//!     has no doorknobs.
//!   - **Ingested.** *"A creature must swallow an entire dose."* A
//!     dinner, not a fight.
//!   - **Inhaled.** *"Blowing the powder or releasing the gas subjects
//!     creatures in a 5-foot Cube to its effect. The resulting cloud
//!     dissipates immediately afterward."* The one other type with a
//!     board surface, and the one shape the zone layer would carry
//!     almost unaltered — a radius-0 area that fires once and goes. It
//!     is not here yet, and it is named rather than forgotten.
//!
//! ## Why the injury four are the ones worth having
//!
//! Because the engine already had exactly one of them and could not
//! say so. The **Dagger of Venom** is an injury poison welded to a
//! blade: a Bonus Action coats it, the coating is spent by the hit that
//! carries it, and what the victim gets is a Constitution save against
//! a number the *item* names. That is the whole of RAW's injury lane,
//! written once, for one dagger, at rarity Rare.
//!
//! These four unweld it. A vial is 150 to 2,000 gold of consumable that
//! any weapon can carry, which is a different decision from a magic
//! item: the dagger is a thing you have, and a vial is a thing you
//! spend. The ladder is the point —
//!
//! | poison | DC | on a failed save |
//! |--------|----|------------------|
//! | Serpent Venom | 11 | 3d6 poison, half on a save |
//! | Spider's Sting | 13 | Poisoned for an hour |
//! | Wyvern Poison | 14 | 7d6 poison, half on a save |
//! | Purple Worm Poison | 21 | 10d6 poison, half on a save |
//!
//! — and it is a ladder in *two* directions at once, which is what
//! makes the choice interesting. Purple Worm Poison is three times
//! Serpent Venom's dice behind a DC almost nothing on the board can
//! make; Spider's Sting rolls no damage at all and is the only one of
//! the four that is still doing something on the fourth round.
//!
//! ## The clause that does not ship
//!
//! Spider's Sting's second sentence: *"If the creature fails the save
//! by 5 or more, the creature also has the Unconscious condition."*
//! The engine's `SaveOutcome` is a pass or a fail and carries no
//! margin — every save in the engine is asked the same yes-or-no
//! question, at the one chokepoint that rolls them — so a clause about
//! *how badly* somebody failed has nowhere to be read. Adding the
//! margin is a change to the shape of every saving throw in the game
//! for one sentence in one vial, and the safe direction is to ship the
//! hour of Poisoned that the sentence is a rider on.
//!
//! ## Where the halves of a poison live
//!
//! Three files, and the split is the same one every magic weapon in the
//! armoury already makes:
//!
//!   - **Here**: the row, in the shape the book prints it, plus
//!     [`Poison::rider`], which is the whole conversion into the
//!     attack layer.
//!   - [`crate::items::item_template`]: the vial, so somebody can find
//!     one.
//!   - [`crate::actions::item_actions`]: the Bonus Action that coats
//!     the blade, on the same `SelfConditionItem` chassis a potion
//!     uses — because applying a poison and drinking a potion are the
//!     same three sentences with a different verb.
//!
//! The marker condition is what ties them together, exactly as
//! `Condition::Envenomed` ties the Dagger of Venom's three halves
//! together. A wielder carrying one has a coated weapon; the rider row
//! keyed to it is what the coating *does*; and `consume_on_trigger`
//! is what "remains potent until delivered through a wound" means on a
//! board where the wound is an attack roll.

use crate::conditions::{Condition, ConditionTimer};
use crate::engine::attack::{FollowUpEffect, OnHitRider, RiderDamage, RiderLane, SmiteFollowUp};
use crate::engine::dice::Dice;
use crate::engine::saves::SaveDamagePolicy;
use crate::engine::types::{AbilityScoreType, DamageType};

/// One sample poison from SRD 5.2's *Poison*, in the shape the book
/// prints it.
///
/// Every field is a clause of the entry, so a row can be checked
/// against the page line by line — the same bargain
/// [`crate::engine::traps::Trap`] makes, and for the same reason: the
/// alternative is hand-written `OnHitRider` literals, where a poison
/// that is one number different from another poison is fourteen lines
/// different here.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Poison {
    /// The poison's name as printed. Doubles as the rider's log tag.
    pub name: &'static str,
    /// The vial as it appears in an inventory, and the name the apply
    /// action gates on. Carried on the row rather than derived from
    /// `name` because the two really are different strings — "Vial of
    /// Wyvern Poison" is an object and "wyvern poison" is what is in
    /// it.
    pub vial_name: &'static str,
    /// The marker a coated weapon carries. One per poison, because
    /// `ON_HIT_RIDERS` is keyed by condition and a shared marker would
    /// make every vial deal the strongest one's dice.
    pub marker: Condition,
    /// RAW's Constitution save DC. Printed by the poison, never
    /// derived from whoever applied it — a vial is as deadly in a
    /// commoner's hand as in an assassin's, which is the difference
    /// between a poison and a spell.
    pub dc: i32,
    /// What a failed save costs in dice, or `Dice::new(0, 0)` for the
    /// rows whose whole effect is the condition.
    pub damage: Dice,
    /// What a failed save installs beside the damage, or `None`.
    pub condition: Option<(Condition, ConditionTimer)>,
    /// RAW's *"or half as much damage on a successful one"*, for the
    /// rows that print it. Every damaging poison in the book does;
    /// Spider's Sting, which deals none, is `NoneOnSave` because there
    /// is nothing to halve.
    pub policy: SaveDamagePolicy,
    /// The book's suggested price for a single dose, in gold.
    ///
    /// Carried because it is the one number that says what a row is
    /// *worth* relative to its neighbours, and because the ladder it
    /// describes is the design: Purple Worm Poison costs thirteen times
    /// Serpent Venom and hits for a bit over three times as much, which
    /// is what a DC of 21 is being charged for.
    pub price_gp: u32,
}

impl Poison {
    /// This poison as a row on [`crate::engine::attack::ON_HIT_RIDERS`].
    ///
    /// The conversion is total — every mechanical field above lands
    /// somewhere — which is the point of keeping the row in the book's
    /// shape. Three columns are the same on all four and are the whole
    /// of what "injury poison" means mechanically:
    ///
    ///   - `dice` is empty, because a poison's damage is behind the
    ///     save and not on the hit. The same reading as the Dagger of
    ///     Venom's, and the same consequence: a critical hit doubles
    ///     the *swing*, and nothing in the rules doubles the poison a
    ///     creature failed a Constitution save against.
    ///   - `consume_on_trigger` is true — *"remains potent until
    ///     delivered through a wound or washed off"*. One coated swing
    ///     per dose, and the dose is gone whether or not the save was
    ///     made, because the poison went into the wound either way.
    ///   - `lane` is `AnyWeapon`. RAW's delivery clause is about the
    ///     damage type rather than the range — *"takes Piercing or
    ///     Slashing damage from an object coated with the poison"* —
    ///     and a coated arrow is as much RAW's case as a coated dagger.
    ///     The piercing-or-slashing half is charged at the *other* end,
    ///     on the Bonus Action that applies it: a mace is not something
    ///     you can smear poison onto usefully, and refusing the
    ///     application is both the simpler gate and the one that tells
    ///     the player before they spend the vial. See
    ///     `item_actions::ApplyPoison`.
    pub const fn rider(&self) -> OnHitRider {
        OnHitRider {
            condition: self.marker,
            dice: Dice::new(0, 0),
            label: self.name,
            damage_type: RiderDamage::Fixed(DamageType::Poison),
            lane: RiderLane::AnyWeapon,
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                save_ability: Some(AbilityScoreType::Constitution),
                // Never read — `fixed_dc` is what a printed poison has
                // — but the field is not an `Option` and Constitution
                // is the ability the clause is about.
                dc_ability: AbilityScoreType::Constitution,
                fixed_dc: Some(self.dc),
                effect: FollowUpEffect::Damage {
                    dice: self.damage,
                    damage_type: DamageType::Poison,
                    condition: self.condition,
                    policy: self.policy,
                },
                label: self.name,
                hp_threshold: None,
                size_cap: None,
                on_success: None,
            }),
            once_per_turn_tag: None,
            target_gate: None,
            requires_natural_twenty: false,
            attacker_gate: None,
            attacker_link: None,
            spends_item_charge: None,
        }
    }
}

/// **Serpent Venom** (200 GP) — *Injury Poison*.
///
/// > *"A creature subjected to Serpent Venom must succeed on a DC 11
/// > Constitution saving throw, taking 10 (3d6) Poison damage on a
/// > failed save or half as much damage on a successful one."*
///
/// The bottom rung, and the one whose DC is low enough that a fighter
/// makes it more often than not. Worth carrying anyway for what it does
/// to the back rank: 3d6 behind a Constitution save is a bad trade
/// against a barbarian and a very good one against a wizard.
pub const SERPENT_VENOM: Poison = Poison {
    name: "serpent venom",
    vial_name: "Vial of Serpent Venom",
    marker: Condition::CoatedSerpentVenom,
    dc: 11,
    damage: Dice::new(3, 6),
    condition: None,
    policy: SaveDamagePolicy::HalfOnSave,
    price_gp: 200,
};

/// **Spider's Sting** (200 GP) — *Injury Poison*.
///
/// > *"A creature subjected to Spider's Sting must succeed on a DC 13
/// > Constitution saving throw or have the Poisoned condition for 1
/// > hour."*
///
/// The odd one out on every axis, and the reason the table is worth
/// having rather than a single scaling number. It rolls no damage at
/// all; what it sells is an hour of disadvantage on every attack roll
/// the victim makes, for the same 200 gold Serpent Venom charges for
/// 3d6. Against anything that will be swinging for more than three
/// rounds it is the better buy, and against anything that is nearly
/// dead it does nothing whatsoever.
///
/// RAW's Unconscious rider — *"if the creature fails the save by 5 or
/// more"* — does not ship. See the module docs for why the margin of a
/// failed save is not a thing this engine can ask about.
pub const SPIDERS_STING: Poison = Poison {
    name: "spider's sting",
    vial_name: "Vial of Spider's Sting",
    marker: Condition::CoatedSpidersSting,
    dc: 13,
    damage: Dice::new(0, 0),
    // RAW's hour, in the hundred rounds the engine spells an hour as —
    // longer than any fight, which is the point: this one does not run
    // out, it is simply survived.
    condition: Some((Condition::Poisoned, ConditionTimer::Rounds(100))),
    // Nothing to halve, and saying so explicitly rather than by
    // accident: a made save against this poison leaves nothing at all.
    policy: SaveDamagePolicy::NoneOnSave,
    price_gp: 200,
};

/// **Wyvern Poison** (1,200 GP) — *Injury Poison*.
///
/// > *"A creature subjected to Wyvern Poison makes a DC 14 Constitution
/// > saving throw, taking 24 (7d6) Poison damage on a failed save or
/// > half as much damage on a successful one."*
///
/// Six times Serpent Venom's price for a bit over twice its dice and
/// three points of DC, which is the shape of every step on this ladder:
/// the damage climbs linearly and the price climbs with the *odds*.
pub const WYVERN_POISON: Poison = Poison {
    name: "wyvern poison",
    vial_name: "Vial of Wyvern Poison",
    marker: Condition::CoatedWyvernPoison,
    dc: 14,
    damage: Dice::new(7, 6),
    condition: None,
    policy: SaveDamagePolicy::HalfOnSave,
    price_gp: 1_200,
};

/// **Purple Worm Poison** (2,000 GP) — *Injury Poison*.
///
/// > *"A creature subjected to Purple Worm Poison makes a DC 21
/// > Constitution saving throw, taking 35 (10d6) Poison damage on a
/// > failed save or half as much damage on a successful one."*
///
/// The most expensive consumable in the book that is not a potion, and
/// the only save DC on this table that an adventuring party mostly
/// fails. Half of 10d6 is still 17 points on the swing that lands it,
/// which is the clause that makes the price make sense: a Purple Worm
/// Poison is the only one of the four that is worth applying to a
/// target you *expect* to make its save.
pub const PURPLE_WORM_POISON: Poison = Poison {
    name: "purple worm poison",
    vial_name: "Vial of Purple Worm Poison",
    marker: Condition::CoatedPurpleWormPoison,
    dc: 21,
    damage: Dice::new(10, 6),
    condition: None,
    policy: SaveDamagePolicy::HalfOnSave,
    price_gp: 2_000,
};

/// The injury poisons, in the order the book prints them.
///
/// Read by `ON_HIT_RIDERS` to build the four rows, and by the sweeps
/// that check each row has a vial somebody can find and an action
/// somebody can spend it with.
pub const ALL_POISONS: [Poison; 4] = [
    SERPENT_VENOM,
    SPIDERS_STING,
    WYVERN_POISON,
    PURPLE_WORM_POISON,
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Every row converts to a rider that can actually cost a creature
    /// something.
    ///
    /// The sweep the trap table carries, asked of the neighbouring
    /// layer: a poison with no dice and no condition is a Bonus Action
    /// and a vial spent on a log line, and nothing in the engine
    /// complains about one.
    #[test]
    fn every_poison_can_cost_a_creature_something() {
        for poison in ALL_POISONS {
            assert!(
                poison.damage.count > 0 || poison.condition.is_some(),
                "{} is applied and does nothing",
                poison.name
            );
        }
    }

    /// A poison whose damage is nothing cannot print "or half as much",
    /// and a poison that deals dice does.
    ///
    /// The two halves of the same mistake, and both are silent: a
    /// `HalfOnSave` on Spider's Sting would halve a zero, and a
    /// `NoneOnSave` on Wyvern Poison would quietly delete RAW's
    /// commonest sentence from the deadliest rows on the table.
    #[test]
    fn the_damaging_poisons_are_the_ones_that_halve() {
        for poison in ALL_POISONS {
            let halves = poison.policy == SaveDamagePolicy::HalfOnSave;
            assert_eq!(
                halves,
                poison.damage.count > 0,
                "{} disagrees with itself about whether a made save leaves anything",
                poison.name
            );
        }
    }

    /// The DCs and the prices climb together.
    ///
    /// Not a rule of the game — it is a fact about the four entries the
    /// book happens to print on this lane — but it is the fact the
    /// ladder is built out of, and a row typo'd to DC 12 instead of 21
    /// would be a 2,000 GP vial that is worse than the 200 GP one.
    #[test]
    fn the_ladder_climbs() {
        let mut by_price = ALL_POISONS;
        by_price.sort_by_key(|p| p.price_gp);
        for pair in by_price.windows(2) {
            assert!(
                pair[1].dc >= pair[0].dc,
                "{} costs more than {} and asks less of its victim",
                pair[1].name,
                pair[0].name
            );
        }
    }

    /// Every row's marker is its own. A shared one would make the
    /// cheapest vial deal the dearest vial's dice, because
    /// `ON_HIT_RIDERS` is keyed by condition and would find whichever
    /// row it walked into first.
    #[test]
    fn no_two_poisons_share_a_marker() {
        let mut seen: Vec<Condition> = Vec::new();
        for poison in ALL_POISONS {
            assert!(
                !seen.contains(&poison.marker),
                "{} shares a marker with another poison",
                poison.name
            );
            seen.push(poison.marker);
        }
    }

    /// The conversion into the attack layer keeps the numbers it was
    /// given, and keeps the three columns that are what "injury poison"
    /// means: no damage on the hit itself, one swing per dose, and a
    /// DC the vial names rather than one its user's spellcasting
    /// derives.
    #[test]
    fn the_rider_is_the_row() {
        for poison in ALL_POISONS {
            let rider = poison.rider();
            assert_eq!(rider.condition, poison.marker);
            assert_eq!(rider.dice.count, 0, "{}: a poison is not a smite", poison.name);
            assert!(
                rider.consume_on_trigger,
                "{} should be spent by the wound that carries it",
                poison.name
            );
            let follow = rider
                .follow_up
                .unwrap_or_else(|| panic!("{} has no save behind it", poison.name));
            assert_eq!(follow.fixed_dc, Some(poison.dc));
            assert_eq!(
                follow.save_ability,
                Some(AbilityScoreType::Constitution),
                "{}: every poison in the book is a Constitution save",
                poison.name
            );
        }
    }
}
