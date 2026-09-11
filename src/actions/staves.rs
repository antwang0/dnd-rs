//! **Staves** — the one SRD 5.2 magic-item category the loot table had
//! no shelf for, and the reason it had none.
//!
//! Every other charge-bearing item in the book is a wand, and a wand is
//! one spell: *"expend 1 of its charges to cast Fireball"*. The engine
//! models those with the config structs in [`crate::actions::item_actions`]
//! — a `AreaSaveDamageItem` naming a dice pool, a DC and a radius —
//! which is a re-statement of the spell, close enough for one entry and
//! wrong to write nine times.
//!
//! A staff is a menu:
//!
//! > *"You can use an action to expend 1 or more of the staff's charges
//! > to cast one of the following spells from it, using your spell save
//! > DC: burning hands (1 charge), fireball (3 charges), or wall of fire
//! > (4 charges)."*
//!
//! Three sentences of RAW, and three things in it the engine could not
//! say before this file:
//!
//! 1. **One item, several actions.** `Item::on_use` is a slice now
//!    rather than an `Option`, so the Staff of Fire contributes three
//!    rows to the picker and one entry to the pack.
//! 2. **A price that is not a slot.** `Resource::ItemCharges` puts the
//!    charge cost in `cost()` where the rest of the engine already looks
//!    — the picker greys the Fireball row out at two charges left and
//!    says why, and `Action::execute`'s billing tail spends them.
//! 3. **The spell, not a copy of it.** [`StaffSpell`] wraps the actual
//!    `&'static` spell every caster in the game uses and swaps out only
//!    what the staff changes: how it is paid for. A Staff of Fire's
//!    Fireball *is* `spells::FIREBALL` — same dice, same DC off the
//!    holder's own spellcasting ability ("using your spell save DC",
//!    which is what RAW asks for), same evasion, same Careful Spell,
//!    same everything, for free and forever.
//!
//! ## What a staff is not
//!
//! **Attunement is not modeled**, here or anywhere: the engine has no
//! between-fights phase for it to happen in, and every item in the file
//! is already live the moment it is picked up.
//!
//! **Recharging is not at dawn.** RAW's *"regains 1d6+1 expended charges
//! daily at dawn"* is a long rest here, which is the engine's only
//! between-fights clock — see `Item::recharge`. A staff emptied in one
//! room comes back partly filled in the next.
//!
//! **The destruction clauses are not modeled.** Six of these staves
//! print a "roll a d20 when you expend the last charge; on a 1 it
//! crumbles" tail, and the Staff of Power prints a whole Retributive
//! Strike. They are all endings for an object that outlives the
//! encounter, and nothing here does.
//!
//! **The staves that are weapons are only half here.** The Staff of
//! Power and the Staff of the Woodlands are also `+2` quarterstaffs;
//! that half rides `ItemBonuses` on the item, which the engine applies
//! to every swing rather than to swings with the staff. See each item's
//! docstring in [`crate::items::item_template`].
//!
//! **Two of the nine staves cast nothing**, and take a second chassis.
//! The Staff of Striking and the Staff of Withering spend their charges
//! sweetening a melee swing, which is the on-hit-rider lane in
//! `engine::attack`: [`StaffPrime`] is the Bonus Action that puts the
//! marker up, and the row in `ON_HIT_RIDERS` that reads it is what pays
//! out. Same pool, same `Resource::ItemCharges`, different half of the
//! engine.

use std::collections::HashSet;

use crate::actions::action_template::{Action, TargetingSchema};
use crate::conditions::{Condition, ConditionTimer};
use crate::engine::action_overrides::ActionOverride;
use crate::engine::encounter::EncounterInstance;
use crate::engine::side_effects::{ApplicableSideEffect, ApplyCondition, Resource};
use crate::engine::types::{Coordinate, DamageType};

/// One row of a staff's spell menu: a spell, the staff it is cast from,
/// and the price in charges.
///
/// Everything a staff option is asked about — where it reaches, what it
/// targets, whether it is harmful, what it installs, whether it holds
/// concentration, what the AI should expect it to do — is the spell's
/// answer, forwarded. Only three things are the staff's:
///
///   - [`Action::cost`], which trades the spell's slot for charges;
///   - [`Action::cast_frame_level`], which keeps the printed level even
///     though nothing paid a slot for it (without this, a staff's
///     Fireball opens a level-0 cast frame and every cantrip-gated
///     feature in the engine mistakes it for one);
///   - [`Action::scales_with_slot`], which is `false` on every staff
///     option even for a spell that scales: the price is fixed in
///     charges, so there is no bigger slot to spend.
///
/// The overrides the caller passes are deliberately **not** forwarded to
/// the spell. `ActionOverride::CastLevel` is the upcast channel, and a
/// staff has nothing to upcast with — RAW prints one level per row and
/// charges for it. Dropping them at this boundary is what makes
/// `cast_frame_level`'s fixed answer true rather than merely usual.
pub struct StaffSpell {
    /// Player-facing name, and the handle the picker sorts under.
    /// Convention: `"<staff>: <spell>"`, so a holder's rows group.
    pub action_name: &'static str,
    /// Shorthands. Kept distinct from the underlying spell's own aliases
    /// — a wizard holding a Staff of Fire has both on their list, and
    /// `prompt::resolve_action` refuses a collision rather than guessing.
    pub action_aliases: &'static [&'static str],
    /// The staff, by the name the charge ledger is keyed on.
    pub item_name: &'static str,
    /// Charges this row costs. RAW's per-spell price.
    pub charges: u32,
    /// The level the spell is cast at, for the cast frame. RAW's printed
    /// level for the row — a staff never casts anything at a level the
    /// item did not choose.
    pub spell_level: u32,
    /// The spell itself, reached through a function because every spell
    /// in `actions::spells` is a `LazyLock` and a `static` initializer
    /// cannot deref one.
    pub spell: fn() -> &'static (dyn Action + Send + Sync),
}

impl StaffSpell {
    fn spell(&self) -> &'static (dyn Action + Send + Sync) {
        (self.spell)()
    }

    /// The charge price as a resource — the one thing every staff row
    /// adds to its spell's cost.
    pub fn charge_cost(&self) -> Resource {
        Resource::ItemCharges {
            item: self.item_name,
            count: self.charges,
        }
    }
}

impl Action for StaffSpell {
    fn name(&self) -> &str {
        self.action_name
    }

    fn aliases(&self) -> Vec<&str> {
        self.action_aliases.to_vec()
    }

    // ---- the spell's answers, forwarded ----

    fn targeting_schema(&self) -> TargetingSchema {
        self.spell().targeting_schema()
    }

    fn reach_tiles(&self) -> Option<isize> {
        self.spell().reach_tiles()
    }

    fn self_burst_radius(&self) -> Option<isize> {
        self.spell().self_burst_radius()
    }

    fn affects_creature(&self, target: &crate::actors::actor_template::ActorInstance) -> bool {
        self.spell().affects_creature(target)
    }

    fn installs_condition(&self) -> Option<crate::conditions::Condition> {
        self.spell().installs_condition()
    }

    fn recharge_key(&self) -> Option<&'static str> {
        self.spell().recharge_key()
    }

    fn spares_allies(&self) -> bool {
        self.spell().spares_allies()
    }

    fn requires_los(&self) -> bool {
        self.spell().requires_los()
    }

    fn is_harmful(&self) -> bool {
        self.spell().is_harmful()
    }

    fn is_melee_attack(&self) -> bool {
        self.spell().is_melee_attack()
    }

    fn min_effective_reach(&self) -> Option<isize> {
        self.spell().min_effective_reach()
    }

    fn normal_range(&self) -> Option<isize> {
        self.spell().normal_range()
    }

    fn is_weapon_attack(&self) -> bool {
        self.spell().is_weapon_attack()
    }

    fn underwater_weapon_name(&self) -> &str {
        self.spell().underwater_weapon_name()
    }

    fn is_light_melee_weapon(&self) -> bool {
        self.spell().is_light_melee_weapon()
    }

    fn is_offhand_swing(&self) -> bool {
        self.spell().is_offhand_swing()
    }

    fn weapon_mastery(&self) -> Option<crate::engine::mastery::WeaponMastery> {
        self.spell().weapon_mastery()
    }

    fn chains_multiple_attacks(&self) -> bool {
        self.spell().chains_multiple_attacks()
    }

    fn hasted_action_eligible(&self) -> bool {
        self.spell().hasted_action_eligible()
    }

    fn deals_damage(&self) -> bool {
        self.spell().deals_damage()
    }

    fn is_heal(&self) -> bool {
        self.spell().is_heal()
    }

    fn cures_conditions(&self) -> &'static [crate::conditions::Condition] {
        self.spell().cures_conditions()
    }

    fn pulses_ally_buff(&self) -> bool {
        self.spell().pulses_ally_buff()
    }

    fn summons_allies(&self) -> bool {
        self.spell().summons_allies()
    }

    fn summons_combatants(&self) -> bool {
        self.spell().summons_combatants()
    }

    fn holds_concentration(&self) -> bool {
        self.spell().holds_concentration()
    }

    fn damage_types(&self) -> Vec<DamageType> {
        self.spell().damage_types()
    }

    fn chooses_damage_type(&self) -> bool {
        self.spell().chooses_damage_type()
    }

    fn expected_damage(&self, encounter: &EncounterInstance, caster_id: usize) -> Option<f32> {
        self.spell().expected_damage(encounter, caster_id)
    }

    fn school(&self) -> Option<crate::engine::types::SpellSchool> {
        self.spell().school()
    }

    // ---- the four the staff answers for itself ----

    /// The spell's action-economy price, with its slot replaced by the
    /// staff's charges.
    ///
    /// Derived from the spell rather than hardcoded to `action_only()`,
    /// so a bonus-action spell stays a bonus action. RAW's staves all say
    /// "use an action", and every row this file ships is an action spell
    /// anyway — but the derivation is the honest one, and a staff row
    /// added for Healing Word should not silently cost more than the
    /// spell does.
    fn cost(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        let mut costs: Vec<Resource> = self
            .spell()
            .cost(encounter, caster_id, target_ids, target_locations, None)
            .into_iter()
            .filter(|r| !matches!(r, Resource::SpellSlot(_)))
            .collect();
        // A cantrip on a staff would arrive here with an Action already
        // in the list; a leveled spell whose whole cost was the slot
        // would arrive empty. Neither is a row that should be free.
        if !costs
            .iter()
            .any(|r| matches!(r, Resource::Action | Resource::BonusAction))
        {
            costs.push(Resource::Action);
        }
        costs.push(self.charge_cost());
        costs
    }

    /// Fixed at the row's printed level. See the struct docstring — a
    /// staff cast has no slot for the default sniff to find, and level 0
    /// is what the engine calls a cantrip.
    fn cast_frame_level(
        &self,
        _encounter: &EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> u32 {
        self.spell_level
    }

    /// Never. The price is charges, and there is no bigger charge.
    fn scales_with_slot(&self) -> bool {
        false
    }

    /// The spell's own gate, and only that.
    ///
    /// "And you are still holding a staff with the charges on it" is
    /// deliberately **not** re-checked here, though every other item
    /// action in the engine re-checks its own item at this hook. It does
    /// not need to be: `validate_input` walks every entry of `cost()`
    /// through `can_consume_resource` before it reaches this method, and
    /// the charge price is an entry of `cost()`. That is the whole
    /// difference a first-class resource makes — a price named in the
    /// cost list is checked by the engine at every gate that checks
    /// prices, and a price paid inside `side_effects` is checked only
    /// where its author remembered to ask.
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        self.spell().custom_validate_input(
            encounter,
            caster_id,
            target_ids,
            target_locations,
            None,
        )
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        self.spell()
            .side_effects(encounter, caster_id, target_ids, target_locations, None)
    }
}

/// Every staff row in the file, in staff order. The set the invariant
/// tests sweep and the AI's staff rung reads — a row written and left
/// off this list is a row nothing can find.
pub static STAFF_SPELLS: &[&StaffSpell] = &[
    &STAFF_OF_FIRE_BURNING_HANDS,
    &STAFF_OF_FIRE_FIREBALL,
    &STAFF_OF_FIRE_WALL_OF_FIRE,
    &STAFF_OF_FROST_FOG_CLOUD,
    &STAFF_OF_FROST_ICE_STORM,
    &STAFF_OF_FROST_WALL_OF_ICE,
    &STAFF_OF_FROST_CONE_OF_COLD,
    &STAFF_OF_HEALING_CURE_WOUNDS,
    &STAFF_OF_HEALING_LESSER_RESTORATION,
    &STAFF_OF_HEALING_MASS_CURE_WOUNDS,
    &STAFF_OF_SWARMING_INSECTS_GIANT_INSECT,
    &STAFF_OF_SWARMING_INSECTS_INSECT_PLAGUE,
    &STAFF_OF_CHARMING_CHARM_PERSON,
    &STAFF_OF_CHARMING_COMMAND,
    &STAFF_OF_THE_WOODLANDS_BARKSKIN,
    &STAFF_OF_THE_WOODLANDS_SPIKE_GROWTH,
    &STAFF_OF_THE_WOODLANDS_WALL_OF_THORNS,
    &STAFF_OF_POWER_MAGIC_MISSILE,
    &STAFF_OF_POWER_RAY_OF_ENFEEBLEMENT,
    &STAFF_OF_POWER_LEVITATE,
    &STAFF_OF_POWER_FIREBALL,
    &STAFF_OF_POWER_LIGHTNING_BOLT,
    &STAFF_OF_POWER_CONE_OF_COLD,
    &STAFF_OF_POWER_HOLD_MONSTER,
    &STAFF_OF_POWER_WALL_OF_FORCE,
    &STAFF_OF_POWER_GLOBE_OF_INVULNERABILITY,
];

// ---------------------------------------------------------------------
// Staff of Fire — 10 charges, RAW's three rows.
// ---------------------------------------------------------------------

pub const STAFF_OF_FIRE_NAME: &str = "Staff of Fire";

pub static STAFF_OF_FIRE_BURNING_HANDS: StaffSpell = StaffSpell {
    action_name: "staff of fire: burning hands",
    action_aliases: &["staff-burning-hands"],
    item_name: STAFF_OF_FIRE_NAME,
    charges: 1,
    spell_level: 1,
    spell: || &*crate::actions::spells::BURNING_HANDS,
};

pub static STAFF_OF_FIRE_FIREBALL: StaffSpell = StaffSpell {
    action_name: "staff of fire: fireball",
    action_aliases: &["staff-fireball"],
    item_name: STAFF_OF_FIRE_NAME,
    charges: 3,
    spell_level: 3,
    spell: || &*crate::actions::spells::FIREBALL,
};

pub static STAFF_OF_FIRE_WALL_OF_FIRE: StaffSpell = StaffSpell {
    action_name: "staff of fire: wall of fire",
    action_aliases: &["staff-wall-of-fire"],
    item_name: STAFF_OF_FIRE_NAME,
    charges: 4,
    spell_level: 4,
    spell: || &*crate::actions::spells::WALL_OF_FIRE,
};

// ---------------------------------------------------------------------
// Staff of Frost — 10 charges, RAW's four rows.
// ---------------------------------------------------------------------

pub const STAFF_OF_FROST_NAME: &str = "Staff of Frost";

pub static STAFF_OF_FROST_FOG_CLOUD: StaffSpell = StaffSpell {
    action_name: "staff of frost: fog cloud",
    action_aliases: &["staff-fog-cloud"],
    item_name: STAFF_OF_FROST_NAME,
    charges: 1,
    spell_level: 1,
    spell: || &*crate::actions::spells::FOG_CLOUD,
};

pub static STAFF_OF_FROST_ICE_STORM: StaffSpell = StaffSpell {
    action_name: "staff of frost: ice storm",
    action_aliases: &["staff-ice-storm"],
    item_name: STAFF_OF_FROST_NAME,
    charges: 4,
    spell_level: 4,
    spell: || &*crate::actions::spells::ICE_STORM,
};

pub static STAFF_OF_FROST_WALL_OF_ICE: StaffSpell = StaffSpell {
    action_name: "staff of frost: wall of ice",
    action_aliases: &["staff-wall-of-ice"],
    item_name: STAFF_OF_FROST_NAME,
    charges: 4,
    spell_level: 6,
    spell: || &*crate::actions::spells::WALL_OF_ICE,
};

pub static STAFF_OF_FROST_CONE_OF_COLD: StaffSpell = StaffSpell {
    action_name: "staff of frost: cone of cold",
    action_aliases: &["staff-cone-of-cold"],
    item_name: STAFF_OF_FROST_NAME,
    charges: 5,
    spell_level: 5,
    spell: || &*crate::actions::spells::CONE_OF_COLD,
};

// ---------------------------------------------------------------------
// Staff of Healing — 10 charges.
// ---------------------------------------------------------------------

pub const STAFF_OF_HEALING_NAME: &str = "Staff of Healing";

/// RAW prices Cure Wounds at "1 charge per spell level, up to 4". The
/// engine's staff rows are one price each, so this is the 1-charge base
/// cast; the 4-charge upcast has no channel here for the reason
/// [`StaffSpell`] gives about `CastLevel`.
pub static STAFF_OF_HEALING_CURE_WOUNDS: StaffSpell = StaffSpell {
    action_name: "staff of healing: cure wounds",
    action_aliases: &["staff-cure-wounds"],
    item_name: STAFF_OF_HEALING_NAME,
    charges: 1,
    spell_level: 1,
    spell: || &*crate::actions::spells::CURE_WOUNDS,
};

pub static STAFF_OF_HEALING_LESSER_RESTORATION: StaffSpell = StaffSpell {
    action_name: "staff of healing: lesser restoration",
    action_aliases: &["staff-lesser-restoration"],
    item_name: STAFF_OF_HEALING_NAME,
    charges: 2,
    spell_level: 2,
    spell: || &*crate::actions::spells::LESSER_RESTORATION,
};

pub static STAFF_OF_HEALING_MASS_CURE_WOUNDS: StaffSpell = StaffSpell {
    action_name: "staff of healing: mass cure wounds",
    action_aliases: &["staff-mass-cure-wounds"],
    item_name: STAFF_OF_HEALING_NAME,
    charges: 5,
    spell_level: 5,
    spell: || &*crate::actions::spells::MASS_CURE_WOUNDS,
};

// ---------------------------------------------------------------------
// Staff of Swarming Insects — 10 charges.
// ---------------------------------------------------------------------

pub const STAFF_OF_SWARMING_INSECTS_NAME: &str = "Staff of Swarming Insects";

pub static STAFF_OF_SWARMING_INSECTS_GIANT_INSECT: StaffSpell = StaffSpell {
    action_name: "staff of swarming insects: giant insect",
    action_aliases: &["staff-giant-insect"],
    item_name: STAFF_OF_SWARMING_INSECTS_NAME,
    charges: 4,
    spell_level: 4,
    spell: || &crate::actions::spells::GIANT_INSECT,
};

pub static STAFF_OF_SWARMING_INSECTS_INSECT_PLAGUE: StaffSpell = StaffSpell {
    action_name: "staff of swarming insects: insect plague",
    action_aliases: &["staff-insect-plague"],
    item_name: STAFF_OF_SWARMING_INSECTS_NAME,
    charges: 5,
    spell_level: 5,
    spell: || &*crate::actions::spells::INSECT_PLAGUE,
};

// ---------------------------------------------------------------------
// Staff of Charming — 10 charges.
// ---------------------------------------------------------------------

pub const STAFF_OF_CHARMING_NAME: &str = "Staff of Charming";

pub static STAFF_OF_CHARMING_CHARM_PERSON: StaffSpell = StaffSpell {
    action_name: "staff of charming: charm person",
    action_aliases: &["staff-charm-person"],
    item_name: STAFF_OF_CHARMING_NAME,
    charges: 1,
    spell_level: 1,
    spell: || &*crate::actions::spells::CHARM_PERSON,
};

pub static STAFF_OF_CHARMING_COMMAND: StaffSpell = StaffSpell {
    action_name: "staff of charming: command",
    action_aliases: &["staff-command"],
    item_name: STAFF_OF_CHARMING_NAME,
    charges: 1,
    spell_level: 1,
    spell: || &*crate::actions::spells::COMMAND,
};

// ---------------------------------------------------------------------
// Staff of the Woodlands — 10 charges. RAW's other four rows (Animal
// Friendship, Awaken, Locate Animals or Plants, Speak with Animals) have
// no combat surface and no spell in the engine to point at.
// ---------------------------------------------------------------------

pub const STAFF_OF_THE_WOODLANDS_NAME: &str = "Staff of the Woodlands";

pub static STAFF_OF_THE_WOODLANDS_BARKSKIN: StaffSpell = StaffSpell {
    action_name: "staff of the woodlands: barkskin",
    action_aliases: &["staff-barkskin"],
    item_name: STAFF_OF_THE_WOODLANDS_NAME,
    charges: 2,
    spell_level: 2,
    spell: || &*crate::actions::spells::BARKSKIN,
};

pub static STAFF_OF_THE_WOODLANDS_SPIKE_GROWTH: StaffSpell = StaffSpell {
    action_name: "staff of the woodlands: spike growth",
    action_aliases: &["staff-spike-growth"],
    item_name: STAFF_OF_THE_WOODLANDS_NAME,
    charges: 2,
    spell_level: 2,
    spell: || &*crate::actions::spells::SPIKE_GROWTH,
};

pub static STAFF_OF_THE_WOODLANDS_WALL_OF_THORNS: StaffSpell = StaffSpell {
    action_name: "staff of the woodlands: wall of thorns",
    action_aliases: &["staff-wall-of-thorns"],
    item_name: STAFF_OF_THE_WOODLANDS_NAME,
    charges: 6,
    spell_level: 6,
    spell: || &*crate::actions::spells::WALL_OF_THORNS,
};

// ---------------------------------------------------------------------
// Staff of Power — 20 charges, and the deepest menu in the book.
// ---------------------------------------------------------------------

pub const STAFF_OF_POWER_NAME: &str = "Staff of Power";

pub static STAFF_OF_POWER_MAGIC_MISSILE: StaffSpell = StaffSpell {
    action_name: "staff of power: magic missile",
    action_aliases: &["staff-magic-missile"],
    item_name: STAFF_OF_POWER_NAME,
    charges: 1,
    spell_level: 1,
    spell: || &*crate::actions::spells::MAGIC_MISSILE,
};

pub static STAFF_OF_POWER_RAY_OF_ENFEEBLEMENT: StaffSpell = StaffSpell {
    action_name: "staff of power: ray of enfeeblement",
    action_aliases: &["staff-ray-of-enfeeblement"],
    item_name: STAFF_OF_POWER_NAME,
    charges: 1,
    spell_level: 2,
    spell: || &*crate::actions::spells::RAY_OF_ENFEEBLEMENT,
};

pub static STAFF_OF_POWER_LEVITATE: StaffSpell = StaffSpell {
    action_name: "staff of power: levitate",
    action_aliases: &["staff-levitate"],
    item_name: STAFF_OF_POWER_NAME,
    charges: 2,
    spell_level: 2,
    spell: || &*crate::actions::spells::LEVITATE,
};

pub static STAFF_OF_POWER_FIREBALL: StaffSpell = StaffSpell {
    action_name: "staff of power: fireball",
    action_aliases: &["staff-power-fireball"],
    item_name: STAFF_OF_POWER_NAME,
    charges: 5,
    // RAW: "fireball (5th-level version, 5 charges)". The five charges
    // buy the upcast, not just the cast — which is why this row's level
    // is 5 where the Staff of Fire's is 3.
    spell_level: 5,
    spell: || &*crate::actions::spells::FIREBALL,
};

pub static STAFF_OF_POWER_LIGHTNING_BOLT: StaffSpell = StaffSpell {
    action_name: "staff of power: lightning bolt",
    action_aliases: &["staff-lightning-bolt"],
    item_name: STAFF_OF_POWER_NAME,
    charges: 5,
    spell_level: 5,
    spell: || &*crate::actions::spells::LIGHTNING_BOLT,
};

pub static STAFF_OF_POWER_CONE_OF_COLD: StaffSpell = StaffSpell {
    action_name: "staff of power: cone of cold",
    action_aliases: &["staff-power-cone-of-cold"],
    item_name: STAFF_OF_POWER_NAME,
    charges: 5,
    spell_level: 5,
    spell: || &*crate::actions::spells::CONE_OF_COLD,
};

pub static STAFF_OF_POWER_HOLD_MONSTER: StaffSpell = StaffSpell {
    action_name: "staff of power: hold monster",
    action_aliases: &["staff-hold-monster"],
    item_name: STAFF_OF_POWER_NAME,
    charges: 5,
    spell_level: 5,
    spell: || &*crate::actions::spells::HOLD_MONSTER,
};

pub static STAFF_OF_POWER_WALL_OF_FORCE: StaffSpell = StaffSpell {
    action_name: "staff of power: wall of force",
    action_aliases: &["staff-wall-of-force"],
    item_name: STAFF_OF_POWER_NAME,
    charges: 5,
    spell_level: 5,
    spell: || &*crate::actions::spells::WALL_OF_FORCE,
};

pub static STAFF_OF_POWER_GLOBE_OF_INVULNERABILITY: StaffSpell = StaffSpell {
    action_name: "staff of power: globe of invulnerability",
    action_aliases: &["staff-globe-of-invulnerability"],
    item_name: STAFF_OF_POWER_NAME,
    charges: 6,
    spell_level: 6,
    spell: || &*crate::actions::spells::GLOBE_OF_INVULNERABILITY,
};

// =====================================================================
// The two staves that are weapons rather than spellbooks.
// =====================================================================

/// The Bonus Action that charges a staff whose payout is a melee hit.
///
/// The Staff of Striking and the Staff of Withering do not cast
/// anything: each spends charges to add damage to the wielder's next
/// swing. That is the `ON_HIT_RIDERS` lane in `engine::attack`, which is
/// driven by a marker condition on the *attacker* — so the item's action
/// is a prime, exactly like a paladin declaring a Smite or a Rune Knight
/// invoking a rune, and the rider row is what reads it.
///
/// Structurally this is `item_actions::KindleWeapon` with a price. The
/// differences are the two that matter: the charges are named in
/// `cost()` so the pool gates the prime, and the marker is consumed by
/// the hit rather than burning for the rest of the fight.
///
/// **Dropping the staff does not disarm the prime**, which is the one
/// hole in the model and one the engine already has: a marker is a
/// condition on the wielder, and `ON_HIT_RIDERS` asks about the
/// condition rather than about the pack. `KindleWeapon`'s docstring
/// records the same parting for the Flame Tongue. Closing it properly
/// would mean a rider row that can ask about inventory, which is a
/// column on a forty-row table for the benefit of a wielder who threw
/// away a Very Rare staff mid-swing.
pub struct StaffPrime {
    /// Player-facing action name.
    pub action_name: &'static str,
    /// Picker aliases.
    pub action_aliases: &'static [&'static str],
    /// The staff, by the name the charge ledger is keyed on.
    pub item_name: &'static str,
    /// Charges the prime costs. RAW's per-swing price.
    pub charges: u32,
    /// Marker installed on the wielder, and the condition the staff's
    /// `ON_HIT_RIDERS` row keys off. Also the re-prime guard: the action
    /// refuses while it is already up, so a wielder cannot stack two
    /// charges' worth of the same die onto one swing.
    pub condition: Condition,
    /// Full log line, `{actor}` substituted with the wielder's name.
    pub log_text: &'static str,
}

impl StaffPrime {
    /// The charge price as a resource.
    pub fn charge_cost(&self) -> Resource {
        Resource::ItemCharges {
            item: self.item_name,
            count: self.charges,
        }
    }
}

impl Action for StaffPrime {
    fn name(&self) -> &str {
        self.action_name
    }

    fn aliases(&self) -> Vec<&str> {
        self.action_aliases.to_vec()
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }

    fn is_harmful(&self) -> bool {
        false
    }

    fn deals_damage(&self) -> bool {
        false
    }

    /// The marker, so the AI's buff lanes and the panel can both see what
    /// this action is for.
    fn installs_condition(&self) -> Option<Condition> {
        Some(self.condition)
    }

    /// A Bonus Action and the charges.
    ///
    /// A bonus action rather than `item_actions::item_use_cost`'s
    /// Action-unless-a-Thief, for the reason `KindleWeapon` gives: the
    /// staff is already in the wielder's hand, not in a pack, so the
    /// cost is the command rather than the rummage. RAW prices the
    /// charges on the hit and says nothing about an action at all — a
    /// bonus action is the closest the engine's economy comes to "free,
    /// as part of the swing", and it is what the smites it is modeled on
    /// cost.
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        vec![Resource::BonusAction, self.charge_cost()]
    }

    /// Holding it, not already primed, and able to swing something.
    ///
    /// The charges are checked by `validate_input`'s walk over `cost()`;
    /// see `StaffSpell::custom_validate_input` for why that is enough.
    ///
    /// The third clause is the one a class prime never needed. A Divine
    /// Strike arrives on a cleric who has a mace; a staff arrives on
    /// whoever walked over it, and a caster with nothing but cantrips
    /// would otherwise spend a bonus action and three charges priming a
    /// hit they are never going to make. The rider's lane is
    /// `RiderLane::MeleeWeapon`, so the gate asks the same question that
    /// lane does.
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        encounter.actors.get(&caster_id).is_some_and(|a| {
            a.has_item_named(self.item_name)
                && !a.has_condition(self.condition)
                && a.available_actions().iter().any(|act| {
                    act.is_harmful()
                        && act.deals_damage()
                        && act.is_melee_attack()
                        && act.school().is_none()
                })
        })
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let name = encounter.actor_name(caster_id);
        encounter.log(self.log_text.replace("{actor}", &name));
        // `Permanent`, and consumed by the swing that cashes it — the
        // rider row carries `consume_on_trigger`. A timer would be the
        // wrong shape: RAW's charge is spent on a hit, and a prime that
        // expired on its own would be charges the wielder paid for and
        // lost to the clock.
        vec![Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: self.condition,
            timer: ConditionTimer::Permanent,
        })]
    }
}

pub const STAFF_OF_STRIKING_NAME: &str = "Staff of Striking";

/// Three charges of the ten, for 3d6 force on the next melee hit.
pub static CHARGE_STAFF_OF_STRIKING: StaffPrime = StaffPrime {
    action_name: "charge staff of striking",
    action_aliases: &["staff-striking", "charge staff"],
    item_name: STAFF_OF_STRIKING_NAME,
    charges: 3,
    condition: Condition::StaffStriking,
    log_text: "{actor} pours three charges into the Staff of Striking; it hums.",
};

pub const STAFF_OF_WITHERING_NAME: &str = "Staff of Withering";

/// One charge of three, for 2d10 necrotic and a DC 15 Constitution save
/// on the next melee hit.
pub static CHARGE_STAFF_OF_WITHERING: StaffPrime = StaffPrime {
    action_name: "charge staff of withering",
    action_aliases: &["staff-withering"],
    item_name: STAFF_OF_WITHERING_NAME,
    charges: 1,
    condition: Condition::StaffWithering,
    log_text: "{actor} wakes the rot in the Staff of Withering.",
};

/// Every prime in the file — the [`StaffPrime`] counterpart of
/// [`STAFF_SPELLS`], swept by the same invariants and read by the AI's
/// own staff rung.
pub static STAFF_PRIMES: &[&StaffPrime] =
    &[&CHARGE_STAFF_OF_STRIKING, &CHARGE_STAFF_OF_WITHERING];
