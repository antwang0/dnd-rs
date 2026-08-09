//! 5e **Weapon Mastery** (SRD 5.2 / 2024 PHB) — the eight properties a
//! weapon carries for a wielder trained to use them.
//!
//! Every weapon in the armoury has exactly one mastery property, and
//! the property does nothing at all until somebody who has the Weapon
//! Mastery feature picks the weapon up. That two-part shape is the
//! whole reason this is a module rather than eight scattered riders:
//!
//!   - **The property belongs to the object.** A longsword saps
//!     whoever it is swung at, and it does so identically in a
//!     fighter's hands and a hobgoblin's. So the tag lives on the
//!     weapon literal (`SimpleWeapon::mastery`), beside the die and the
//!     damage type, and every wielder of that static shares it.
//!
//!   - **The training belongs to the creature.** RAW grants Weapon
//!     Mastery to five classes at level 1 and to nobody else, so the
//!     property is gated on `ActorInstance::has_weapon_mastery` at the
//!     one place it fires. A goblin with a scimitar is still a goblin
//!     with a scimitar.
//!
//! The gate is checked here rather than at the weapon literal because
//! the same static is on a PC's action list and on a monster's — the
//! bestiary's `SCIMITAR` *is* the fighter's scimitar — and a per-wielder
//! rule cannot be answered by per-weapon data.
//!
//! ## Where each property fires
//!
//! Six of the eight ride the shared attack pipeline through
//! `MasteryRider`, which is an `ActionOnHitRider` and therefore reaches
//! every rule `engine::attack` applies rather than a private copy of
//! some of them:
//!
//!   - **Graze** is the only one that fires on a *miss*, which is why
//!     the rider trait grew an `on_miss` hook. Ability-modifier damage,
//!     typed as the weapon, on any miss — including a natural 1, since
//!     RAW's trigger is "misses a creature" and a fumble is a miss.
//!   - **Cleave**, **Push**, **Sap**, **Slow**, **Topple** and **Vex**
//!     fire on a hit, in `MasteryRider::apply`.
//!
//! **Nick** is the exception, and it is not an on-hit clause at all: it
//! changes what the off-hand swing *costs*, so it is read by
//! `actions::two_weapon::OffHandAttack::cost` instead. See
//! `nick_is_free` there.
//!
//! ## Once per turn
//!
//! Three properties are RAW-limited to once per turn — Cleave, Nick and
//! Slow — and all three route through the actor's shared
//! `once_per_turn_marks` ledger, which `reset_for_new_round` clears. The
//! tags are `const`s below so the mark and the check can't drift apart.

use crate::actions::action_template::MELEE_REACH;
use crate::conditions::{Condition, ConditionTimer};
use crate::engine::attack::{ActionOnHitRider, AttackParams, AttackRiderPair, resolve_attack};
use crate::engine::dice::RollMode;
use crate::engine::encounter::EncounterInstance;
use crate::engine::side_effects::{
    ApplicableSideEffect, DealDamage, PushActor, install_condition_with_link,
};
use crate::engine::types::{AbilityScoreType, Size};
use crate::engine::util::tiles_from_feet;

/// Once-per-turn ledger tag for the Cleave follow-up swing.
pub const CLEAVE_TAG: &str = "weapon mastery: cleave";
/// Once-per-turn ledger tag for the free Nick off-hand swing.
pub const NICK_TAG: &str = "weapon mastery: nick";
/// Once-per-turn ledger tag for the Slow speed cut.
pub const SLOW_TAG: &str = "weapon mastery: slow";

/// How far Push shoves its target. RAW is 10 feet.
const PUSH_TILES: u32 = tiles_from_feet(10);

/// The eight 5e weapon mastery properties.
///
/// One per weapon, never more — RAW assigns exactly one to each entry in
/// the weapon table — which is why `SimpleWeapon::mastery` is an
/// `Option<WeaponMastery>` and not a set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WeaponMastery {
    /// *"If you hit a creature with a melee attack roll using this
    /// weapon, you can make an attack roll with the weapon against a
    /// second creature within 5 feet of the first that is also within
    /// your reach. On a hit, the second creature takes the weapon's
    /// damage, but don't add your ability modifier to that damage
    /// unless that modifier is negative. You can make this extra attack
    /// only once per turn."*
    ///
    /// The heavy two-handers: greataxe, halberd.
    Cleave,
    /// *"If your attack roll with this weapon misses a creature, you can
    /// deal damage to that creature equal to the ability modifier you
    /// used to make the attack roll. This damage is the same type dealt
    /// by the weapon."*
    ///
    /// The reach two-handers: greatsword, glaive.
    Graze,
    /// *"When you make the extra attack of the Light property, you can
    /// make it as part of the Attack action instead of as a Bonus
    /// Action. You can make this extra attack only once per turn."*
    ///
    /// The small blades: dagger, light hammer, sickle, scimitar.
    Nick,
    /// *"If you hit a creature with this weapon, you can push the target
    /// up to 10 feet straight away from yourself if it is Large or
    /// smaller."*
    ///
    /// The battering weapons: greatclub, pike, warhammer, heavy crossbow.
    Push,
    /// *"If you hit a creature with this weapon, that creature has
    /// Disadvantage on its next attack roll before the start of your
    /// next turn."*
    ///
    /// The disciplined weapons: mace, spear, flail, longsword,
    /// morningstar, war pick.
    Sap,
    /// *"If you hit a creature with this weapon and deal damage to it,
    /// you can reduce its Speed by 10 feet until the start of your next
    /// turn. If the creature is hit more than once by weapons that have
    /// this property, the Speed reduction doesn't exceed 10 feet."*
    ///
    /// The tangling and the distant: club, javelin, whip, sling, light
    /// crossbow, longbow.
    Slow,
    /// *"If you hit a creature with this weapon, you can knock the
    /// target prone if it fails a Constitution saving throw (DC 8 plus
    /// the ability modifier used to make the attack roll and your
    /// Proficiency Bonus)."*
    ///
    /// The levering weapons: quarterstaff, battleaxe, lance, maul,
    /// trident.
    Topple,
    /// *"If you hit a creature with this weapon and deal damage to it,
    /// you have Advantage on your next attack roll against that creature
    /// before the end of your current turn."*
    ///
    /// (The engine's window is slightly longer — see `Condition::Vexed`.)
    ///
    /// The quick weapons: handaxe, dart, shortbow, rapier, shortsword,
    /// blowgun, hand crossbow.
    Vex,
}

impl WeaponMastery {
    /// Short lower-case label — the log prefix and the picker suffix.
    pub const fn name(self) -> &'static str {
        match self {
            WeaponMastery::Cleave => "cleave",
            WeaponMastery::Graze => "graze",
            WeaponMastery::Nick => "nick",
            WeaponMastery::Push => "push",
            WeaponMastery::Sap => "sap",
            WeaponMastery::Slow => "slow",
            WeaponMastery::Topple => "topple",
            WeaponMastery::Vex => "vex",
        }
    }

    /// One-line player-facing summary, shown beside the weapon wherever
    /// the UI has room for it.
    pub const fn blurb(self) -> &'static str {
        match self {
            WeaponMastery::Cleave => "on a hit, a free swing at a second foe beside the first",
            WeaponMastery::Graze => "on a miss, still deal your ability modifier as damage",
            WeaponMastery::Nick => "the off-hand swing rides the Attack action, not a bonus action",
            WeaponMastery::Push => "on a hit, shove the target 10 ft away",
            WeaponMastery::Sap => "on a hit, the target's next attack is at disadvantage",
            WeaponMastery::Slow => "on a hit, the target's speed drops 10 ft",
            WeaponMastery::Topple => "on a hit, a CON save or the target falls prone",
            WeaponMastery::Vex => "on a hit, advantage on your next swing at that target",
        }
    }
}

/// True if `actor_id` is trained to use a weapon's mastery property.
///
/// A free function rather than a method on the rider because three
/// unrelated places ask it — the on-hit rider, the Nick cost lane in
/// `two_weapon`, and the UI's weapon annotation — and none of them
/// should re-derive "does this creature have the feature" from anything
/// but the one flag.
pub fn wields_with_mastery(encounter: &EncounterInstance, actor_id: usize) -> bool {
    encounter
        .actors
        .get(&actor_id)
        .is_some_and(|a| a.has_weapon_mastery())
}

/// The mastery property `actor_id` actually gets from a weapon tagged
/// `mastery` — `None` when the weapon has none or the wielder is
/// untrained.
///
/// Every consumer goes through this rather than reading the tag
/// directly, so the training gate cannot be forgotten at a new call
/// site. That is not hypothetical care: the tag lives on shared statics
/// that monsters swing too, so a missed gate hands the bestiary a
/// feature RAW gives five classes.
pub fn effective_mastery(
    encounter: &EncounterInstance,
    actor_id: usize,
    mastery: Option<WeaponMastery>,
) -> Option<WeaponMastery> {
    mastery.filter(|_| wields_with_mastery(encounter, actor_id))
}

/// The weapon's mastery property, carried onto the shared attack
/// pipeline as an `ActionOnHitRider`.
///
/// Constructed unconditionally by every weapon chassis and inert when
/// `mastery` is `None` or the wielder is untrained, so the swing sites
/// stay one line and the gate stays in one place. `inner` chains the
/// action's *own* rider underneath — the Rogue's Sneak Attack is the
/// case that needs it, since a rogue's shortsword has both a mastery
/// property and a sneak clause and the pipeline takes one rider.
pub struct MasteryRider {
    /// The property this weapon carries, before the training gate.
    pub mastery: Option<WeaponMastery>,
    /// The ability the swing rolled to hit with. Not derivable from
    /// `AttackParams`, which carries the finished modifier rather than
    /// which ability produced it, and both Graze (damage equal to the
    /// modifier) and Topple (DC 8 + modifier + proficiency) need the
    /// ability itself.
    pub ability: AbilityScoreType,
    /// The wielder's reach in tile-gap units — Cleave's second target
    /// must be "within your reach", which for a glaive is further than
    /// for a greataxe.
    pub reach: isize,
}

impl MasteryRider {
    /// The rider a weapon with `mastery` swung with `ability` at
    /// `MELEE_REACH` carries. The common case: every melee weapon on the
    /// roster but the reach ones.
    pub fn new(mastery: Option<WeaponMastery>, ability: AbilityScoreType) -> Self {
        Self {
            mastery,
            ability,
            reach: MELEE_REACH,
        }
    }

    /// `new` with an explicit reach, for the polearms whose Cleave /
    /// Graze envelope is wider than a sword's.
    pub fn with_reach(
        mastery: Option<WeaponMastery>,
        ability: AbilityScoreType,
        reach: isize,
    ) -> Self {
        Self {
            mastery,
            ability,
            reach,
        }
    }

    /// Chain this rider over an action's own — `MasteryRider::new(..).
    /// over(&SneakAttack)`. Both fire, mastery last, and both damage
    /// figures fold into the returned total.
    pub fn over<'a>(self, inner: &'a dyn ActionOnHitRider) -> AttackRiderPair<'a, Self> {
        AttackRiderPair {
            first: inner,
            second: self,
        }
    }
}

impl ActionOnHitRider for MasteryRider {
    fn apply(
        &self,
        encounter: &mut EncounterInstance,
        p: &AttackParams,
        _mode: RollMode,
        _is_crit: bool,
        effects: &mut Vec<Box<dyn ApplicableSideEffect>>,
    ) -> u32 {
        let Some(mastery) = effective_mastery(encounter, p.caster_id, self.mastery) else {
            return 0;
        };
        match mastery {
            WeaponMastery::Cleave => cleave(encounter, p, self.reach, effects),
            WeaponMastery::Push => {
                push(encounter, p, effects);
                0
            }
            WeaponMastery::Sap => {
                sap(encounter, p, effects);
                0
            }
            WeaponMastery::Slow => {
                slow(encounter, p, effects);
                0
            }
            WeaponMastery::Topple => {
                topple(encounter, p, self.ability, effects);
                0
            }
            WeaponMastery::Vex => {
                vex(encounter, p, effects);
                0
            }
            // Fires at the cost lane, not on a hit — see the module
            // docs and `two_weapon::OffHandAttack::cost`.
            WeaponMastery::Nick => 0,
            // Fires on a miss. `on_miss` below.
            WeaponMastery::Graze => 0,
        }
    }

    fn on_miss(
        &self,
        encounter: &mut EncounterInstance,
        p: &AttackParams,
        effects: &mut Vec<Box<dyn ApplicableSideEffect>>,
    ) -> u32 {
        if effective_mastery(encounter, p.caster_id, self.mastery) != Some(WeaponMastery::Graze) {
            return 0;
        }
        graze(encounter, p, self.ability, effects)
    }
}

/// **Graze** — a miss still costs the target the wielder's ability
/// modifier in the weapon's damage type.
///
/// A non-positive modifier deals nothing and logs nothing: RAW's clause
/// is a floor of "the ability modifier", and a Strength-8 greatsword
/// user grazing for -1 would be healing the target.
fn graze(
    encounter: &mut EncounterInstance,
    p: &AttackParams,
    ability: AbilityScoreType,
    effects: &mut Vec<Box<dyn ApplicableSideEffect>>,
) -> u32 {
    let Some(caster) = encounter.actors.get(&p.caster_id) else {
        return 0;
    };
    let amount = caster.ability_modifier(ability).max(0) as u32;
    if amount == 0 {
        return 0;
    }
    encounter.log(format!(
        "  graze: the miss still lands {} {}",
        amount, p.damage_type
    ));
    effects.push(Box::new(DealDamage {
        actor_id: p.target_id,
        amount,
        damage_type: p.damage_type,
    }));
    amount
}

/// **Cleave** — one free swing at a second creature beside the first.
///
/// Three gates, all RAW: melee only, once per turn, and the second
/// creature must be both within 5 feet of the first *and* within the
/// wielder's own reach. The follow-up rolls the weapon's dice with no
/// ability modifier (RAW drops it unless it is negative, which
/// `cleave_damage_bonus` honours), and it carries no rider of its own —
/// a cleave that cleaved would be a chain reaction RAW does not have.
///
/// Returns 0 rather than the follow-up's damage: the pipeline's return
/// value is what *this* target took, and the second creature is not
/// them. Its `DealDamage` rides `effects` like any other payload.
fn cleave(
    encounter: &mut EncounterInstance,
    p: &AttackParams,
    reach: isize,
    effects: &mut Vec<Box<dyn ApplicableSideEffect>>,
) -> u32 {
    if !p.is_melee {
        return 0;
    }
    let Some(caster) = encounter.actors.get(&p.caster_id) else {
        return 0;
    };
    if caster.once_per_turn_used(CLEAVE_TAG) {
        return 0;
    }
    let Some(second) = cleave_target(encounter, p, reach) else {
        return 0;
    };
    if let Some(caster) = encounter.actors.get_mut(&p.caster_id) {
        caster.mark_once_per_turn_used(CLEAVE_TAG);
    }
    let name = encounter
        .actors
        .get(&second)
        .map(|a| a.name().to_string())
        .unwrap_or_default();
    encounter.log(format!("  cleave: the swing carries on into {}", name));
    let damage_bonus = cleave_damage_bonus(p);
    let follow_up = resolve_attack(
        encounter,
        AttackParams {
            caster_id: p.caster_id,
            target_id: second,
            action_name: p.action_name,
            attack_bonus: p.attack_bonus,
            damage_dice: p.damage_dice,
            damage_bonus,
            damage_type: p.damage_type,
            is_melee: true,
            long_range: None,
            min_range: p.min_range,
            is_spell: false,
        },
    );
    effects.extend(follow_up);
    0
}

/// RAW: *"don't add your ability modifier to that damage unless that
/// modifier is negative."* The same asymmetry two-weapon fighting has,
/// and for the same reason — a penalty you cannot decline.
///
/// Read off the primary swing's `damage_bonus` rather than re-derived
/// from the wielder: that field already carries whichever modifier the
/// weapon actually applied — a finesse blade's DEX over its holder's
/// STR, a natural weapon's flat nothing — and asking the actor a second
/// time is a second chance to answer differently.
fn cleave_damage_bonus(p: &AttackParams) -> i32 {
    p.damage_bonus.min(0)
}

/// The creature a cleave carries into: an enemy of the wielder, not the
/// primary target, within 5 feet of it and within the wielder's reach.
///
/// Ties break on the lowest id so a seeded encounter replays identically
/// — the same reason every other multi-candidate sweep in the engine
/// sorts before it picks.
fn cleave_target(encounter: &EncounterInstance, p: &AttackParams, reach: isize) -> Option<usize> {
    let attacker_team = encounter.actors.get(&p.caster_id)?.team();
    let mut candidates: Vec<usize> = encounter
        .actors
        .iter()
        .filter(|(id, a)| {
            **id != p.target_id
                && **id != p.caster_id
                && a.team() != attacker_team
                && a.is_combat_active()
        })
        .map(|(id, _)| *id)
        .filter(|id| {
            encounter
                .footprint_distance(p.target_id, *id)
                .is_some_and(|d| d <= MELEE_REACH)
                && encounter
                    .footprint_distance(p.caster_id, *id)
                    .is_some_and(|d| d <= reach)
        })
        .collect();
    candidates.sort_unstable();
    candidates.first().copied()
}

/// **Push** — a 10-foot shove on a hit, Large or smaller only.
fn push(
    encounter: &mut EncounterInstance,
    p: &AttackParams,
    effects: &mut Vec<Box<dyn ApplicableSideEffect>>,
) {
    let too_big = encounter
        .actors
        .get(&p.target_id)
        .is_none_or(|t| t.size().ordinal() > Size::Large.ordinal());
    if too_big {
        return;
    }
    let Some(from) = encounter.actors.get(&p.caster_id).map(|c| c.location()) else {
        return;
    };
    encounter.log("  push: the blow drives the target back 10 ft");
    effects.push(Box::new(PushActor {
        actor_id: p.target_id,
        from,
        max_tiles: PUSH_TILES,
    }));
}

/// **Sap** — the target's next attack roll is at disadvantage.
///
/// Installed as `Condition::Sapped`, which sits on the
/// `imposes_attacker_disadvantage` cohort and on `CONSUMED_ON_ATTACK`,
/// so it is spent by the target's next swing exactly as RAW says and
/// expires on its own if they never take one.
fn sap(
    encounter: &mut EncounterInstance,
    p: &AttackParams,
    effects: &mut Vec<Box<dyn ApplicableSideEffect>>,
) {
    if encounter
        .actors
        .get(&p.target_id)
        .is_some_and(|t| t.has_condition(Condition::Sapped))
    {
        return;
    }
    encounter.log("  sap: the target is thrown off balance");
    effects.extend(install_condition_with_link(
        Condition::Sapped,
        p.target_id,
        p.caster_id,
        ConditionTimer::Rounds(1),
    ));
}

/// **Slow** — 10 feet off the target's speed until the start of the
/// wielder's next turn.
///
/// Once per turn, which is how RAW's *"the Speed reduction doesn't
/// exceed 10 feet"* clause is enforced here: one install per wielder per
/// turn, and `Condition::Hobbled` contributes its -10 once however many
/// times it is re-applied, because a held condition is a set membership
/// rather than a stack.
fn slow(
    encounter: &mut EncounterInstance,
    p: &AttackParams,
    effects: &mut Vec<Box<dyn ApplicableSideEffect>>,
) {
    let Some(caster) = encounter.actors.get(&p.caster_id) else {
        return;
    };
    if caster.once_per_turn_used(SLOW_TAG) {
        return;
    }
    if let Some(caster) = encounter.actors.get_mut(&p.caster_id) {
        caster.mark_once_per_turn_used(SLOW_TAG);
    }
    encounter.log("  slow: the target's footing gives, 10 ft off its speed");
    effects.extend(install_condition_with_link(
        Condition::Hobbled,
        p.target_id,
        p.caster_id,
        ConditionTimer::Rounds(1),
    ));
}

/// **Topple** — a Constitution save or the target falls prone.
///
/// DC is RAW's *"8 plus the ability modifier used to make the attack
/// roll and your Proficiency Bonus"*, which is the same number as a
/// spell save DC computed off the swing's ability — hence the reuse of
/// `spell_save_dc` rather than a second copy of `8 + mod + prof`.
fn topple(
    encounter: &mut EncounterInstance,
    p: &AttackParams,
    ability: AbilityScoreType,
    effects: &mut Vec<Box<dyn ApplicableSideEffect>>,
) {
    if encounter
        .actors
        .get(&p.target_id)
        .is_some_and(|t| t.has_condition(Condition::Prone))
    {
        return;
    }
    let Some(dc) = encounter
        .actors
        .get(&p.caster_id)
        .map(|c| c.spell_save_dc(ability))
    else {
        return;
    };
    let save = encounter.roll_save(p.target_id, AbilityScoreType::Constitution, dc);
    if save.passed() {
        return;
    }
    encounter.log("  topple: the target is knocked prone");
    effects.extend(install_condition_with_link(
        Condition::Prone,
        p.target_id,
        p.caster_id,
        ConditionTimer::Permanent,
    ));
}

/// **Vex** — advantage on the wielder's next swing at this target.
///
/// Back-linked to the wielder, so the advantage belongs to them and not
/// to whoever else happens to be attacking the same creature. Read by
/// `attack_mode_tally`'s `matched_link_mode` sweep, the same one the
/// Vengeance Paladin's Vow of Enmity rides.
fn vex(
    encounter: &mut EncounterInstance,
    p: &AttackParams,
    effects: &mut Vec<Box<dyn ApplicableSideEffect>>,
) {
    encounter.log("  vex: the target is left open to the next blow");
    effects.extend(install_condition_with_link(
        Condition::Vexed,
        p.target_id,
        p.caster_id,
        ConditionTimer::Rounds(1),
    ));
}
