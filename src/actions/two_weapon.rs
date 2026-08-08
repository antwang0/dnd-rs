//! 5e **two-weapon fighting** — the bonus-action off-hand swing.
//!
//! RAW (PHB "Two-Weapon Fighting"): *"When you take the Attack action
//! and attack with a light melee weapon that you're holding in one
//! hand, you can use a bonus action to attack with a different light
//! melee weapon that you're holding in the other hand. You don't add
//! your ability modifier to the damage of the bonus attack, unless
//! that modifier is negative."*
//!
//! The clause has three moving parts and the engine now models all
//! three:
//!
//!   1. **The opening.** A light melee weapon swung at Action cost
//!      stamps `ActorInstance::light_weapon_swing_this_turn` from the
//!      stack's execution chokepoint. `OffHandAttack` gates on that
//!      flag, so the bonus swing is legal exactly when the main hand
//!      has actually gone in — not merely when the Action slot is
//!      empty. A fighter who Dashed and a wizard who cast Fireball
//!      have both spent their Action and neither has opened anything.
//!   2. **The cost.** One `Resource::BonusAction`, which is what makes
//!      the option a real decision: it is the same slot a Barbarian's
//!      Rage, a Monk's Flurry, a Rogue's Cunning Action and a Cleric's
//!      Spiritual Weapon all want.
//!   3. **The damage.** No ability modifier — the off-hand swing is
//!      dice only — *unless* the wielder carries the Two-Weapon
//!      Fighting **fighting style**, which is the entire content of
//!      that style. RAW's "unless that modifier is negative" tail is
//!      the sole case where the penalty applies without the style;
//!      it is handled by `offhand_damage_ability` rather than being
//!      dropped, because a Strength-8 dual-wielder taking -1 per
//!      off-hand swing is the reason the tail is written.
//!
//! Before this module the style existed as a flag that added the
//! wielder's Strength modifier to *every* melee swing they made, with
//! a comment conceding the collapse and noting no template shipped it.
//! That is a strictly larger buff than RAW's — a Ranger with Extra
//! Attack would have taken it three times a turn for a clause that
//! grants it once — and the flag now means what its name says.

use std::collections::HashSet;

use crate::{
    actions::action_template::{Action, MELEE_REACH, TargetingSchema, first_target_id},
    actors::actor_template::ActorInstance,
    engine::{
        action_overrides::ActionOverride,
        dice::Dice,
        encounter::EncounterInstance,
        side_effects::{ApplicableSideEffect, Resource},
        types::{AbilityScoreType, Coordinate, DamageType},
    },
};

/// Which ability modifier, if any, `wielder` adds to the damage of an
/// off-hand swing.
///
/// `Some(ability)` in exactly two cases, which is RAW read literally:
///
///   - The wielder has the **Two-Weapon Fighting** fighting style,
///     whose whole text is "you can add your ability modifier to the
///     damage of the second attack".
///   - The modifier is **negative**. RAW's exclusion is "you don't add
///     your ability modifier … unless that modifier is negative",
///     which is a penalty you cannot decline. Without this branch a
///     Strength-8 wielder's off-hand swing would hit *harder* than
///     their main hand, since the main hand carries the -1 and the
///     off-hand would not.
///
/// Shared by the swing and its `expected_damage` estimate so the AI's
/// attack picker ranks the off-hand blade by the damage it will
/// actually deal.
fn offhand_damage_ability(
    wielder: &ActorInstance,
    ability: AbilityScoreType,
) -> Option<AbilityScoreType> {
    if wielder.has_two_weapon_fighting_style() || wielder.ability_modifier(ability) < 0 {
        Some(ability)
    } else {
        None
    }
}

/// A bonus-action off-hand weapon swing.
///
/// Data-only, in the shape of `SimpleWeapon` and `RogueWeapon` beside
/// it: the off-hand weapons on the roster differ in name, die and
/// damage type and in nothing else. Everything that makes the swing an
/// *off-hand* swing — the cost, the gate, the modifier rule — is the
/// same for all of them and lives on the impl rather than the literal.
///
/// Reach is pinned to `MELEE_REACH` and there is no ranged variant,
/// deliberately. RAW's clause names a light *melee* weapon in each
/// hand; a hand crossbow's bonus-action shot is the Crossbow Expert
/// feat, which is a different rule with a different gate.
pub struct OffHandAttack {
    /// Display name — action list entry, prompt parser's canonical
    /// name, and the attack log's subject. Conventionally the weapon
    /// prefixed with "off-hand", so the log distinguishes the two
    /// swings a dual-wielder makes in one turn.
    pub display_name: &'static str,
    /// Alias set for the prompt parser.
    pub aliases: &'static [&'static str],
    /// Ability the swing rolls to hit with, and the one whose modifier
    /// the damage roll picks up if `offhand_damage_ability` says so.
    /// Strength for an ordinary off-hand blade; Dexterity for a
    /// finesse one, which is every light weapon a Rogue or Ranger
    /// actually holds.
    pub attack_ability: AbilityScoreType,
    /// The off-hand weapon's die.
    pub damage_dice: Dice,
    /// The off-hand weapon's damage type.
    pub damage_type: DamageType,
}

impl Action for OffHandAttack {
    fn name(&self) -> &str {
        self.display_name
    }

    fn aliases(&self) -> Vec<&str> {
        self.aliases.to_vec()
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }

    fn reach_tiles(&self) -> Option<isize> {
        Some(MELEE_REACH)
    }

    fn requires_los(&self) -> bool {
        false
    }

    fn is_melee_attack(&self) -> bool {
        true
    }

    fn is_weapon_attack(&self) -> bool {
        true
    }

    /// Deliberately **not** a light-weapon opening itself, even though
    /// the weapon in the off hand is light.
    ///
    /// RAW's opening is the *Attack action*, and the flag exists to
    /// answer "may this actor take a bonus swing". Returning `true`
    /// here would have the off-hand swing re-arm its own gate, which
    /// is harmless today — the actor's bonus action is already spent
    /// by then — and would become a second free swing the moment
    /// anything hands out a second bonus action. The write site gates
    /// on Action cost too, so this is belt and braces; both are
    /// cheap.
    fn is_light_melee_weapon(&self) -> bool {
        false
    }

    fn damage_types(&self) -> Vec<DamageType> {
        vec![self.damage_type]
    }

    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        vec![Resource::BonusAction]
    }

    /// RAW's opening clause, and the reason the ledger exists.
    ///
    /// Gating on the ledger rather than on `action_slots() == 0` — the
    /// approximation the Soulknife's second blade uses — matters here
    /// because the off-hand blade sits on the action list of martials
    /// who have plenty of other things to spend an Action on. A Ranger
    /// who cast Hunter's Mark, a Fighter who Dashed to close, a Rogue
    /// who Hid: all three have an empty Action slot and none of them
    /// has a weapon swinging in their main hand for the off hand to
    /// follow.
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        encounter
            .actors
            .get(&caster_id)
            .is_some_and(|a| a.has_swung_light_weapon_this_turn())
    }

    /// The estimate reads the same `offhand_damage_ability` the swing
    /// does, so a wielder without the style is ranked on dice alone —
    /// which is the point of the ranking. An off-hand shortsword that
    /// claimed +DEX would outscore a main-hand longsword on the
    /// picker's damage key and get chosen first, at which point its
    /// own gate would reject it.
    ///
    /// `extra_swings: 0` and a `BonusAction` cost between them keep
    /// Extra Attack out of the number, which is RAW: Extra Attack
    /// multiplies the Attack action, not the bonus swing that follows
    /// it.
    fn expected_damage(&self, encounter: &EncounterInstance, caster_id: usize) -> Option<f32> {
        let caster = encounter.actors.get(&caster_id)?;
        crate::actions::action_template::weapon_expected_damage_named(
            encounter,
            caster_id,
            self.display_name,
            self.damage_dice,
            offhand_damage_ability(caster, self.attack_ability),
            Resource::BonusAction,
            0,
        )
    }

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
        let attack_bonus = caster.spell_attack_modifier(self.attack_ability);
        let damage_bonus = offhand_damage_ability(caster, self.attack_ability)
            .map(|a| caster.ability_modifier(a))
            .unwrap_or(0);
        crate::engine::attack::resolve_attack(
            encounter,
            crate::engine::attack::AttackParams {
                caster_id,
                target_id,
                action_name: self.display_name,
                attack_bonus,
                damage_dice: self.damage_dice,
                damage_bonus,
                damage_type: self.damage_type,
                is_melee: true,
                long_range: None,
                min_range: None,
                is_spell: false,
            },
        )
        // No `maybe_chain_extra_attack`: Extra Attack multiplies the
        // Attack action, and this is not it.
    }
}

/// Off-hand shortsword — DEX-based 1d6 piercing, the finesse blade a
/// dual-wielding Ranger or Rogue actually holds in their second hand.
///
/// Pairs with the `SHORTSWORD` / `ROGUE_SHORTSWORD` / `SCIMITAR` in
/// the main hand; all three are light, so any of them opens it.
pub static OFF_HAND_SHORTSWORD: OffHandAttack = OffHandAttack {
    display_name: "off-hand shortsword",
    aliases: &["oh", "offhand", "off-hand"],
    attack_ability: AbilityScoreType::Dexterity,
    damage_dice: Dice::new(1, 6),
    damage_type: DamageType::Piercing,
};

/// Off-hand scimitar — STR-based 1d6 slashing, for the Strength-build
/// martial whose main hand is already a scimitar.
///
/// Distinct from the shortsword above on ability and damage type
/// rather than on die size, which is the pairing that matters: a
/// wielder resisted for slashing wants the piercing blade in reserve
/// and vice versa.
pub static OFF_HAND_SCIMITAR: OffHandAttack = OffHandAttack {
    display_name: "off-hand scimitar",
    aliases: &["ohs", "offhand scimitar"],
    attack_ability: AbilityScoreType::Strength,
    damage_dice: Dice::new(1, 6),
    damage_type: DamageType::Slashing,
};

/// Off-hand dagger — DEX-based 1d4 piercing. The smallest blade on the
/// roster, and the one a caster-martial hybrid carries: a Bladesinger
/// or an Arcane Trickster is holding a dagger because it is the weapon
/// their class is proficient with, not because it is a good one.
pub static OFF_HAND_DAGGER: OffHandAttack = OffHandAttack {
    display_name: "off-hand dagger",
    aliases: &["ohd", "offhand dagger"],
    attack_ability: AbilityScoreType::Dexterity,
    damage_dice: Dice::new(1, 4),
    damage_type: DamageType::Piercing,
};
