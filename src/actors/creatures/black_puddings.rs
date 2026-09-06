use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::BLACK_PUDDING_PSEUDOPOD;
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::attack::{MeleeReflect, ReflectDamage};
use crate::engine::dice::Dice;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Black Pudding **Corrosive Form** — the iconic ooze passive: any
/// creature that hits the pudding with a melee attack takes 1d8 acid
/// damage from the splash-back of the corrosive sludge. Wires through
/// the engine's natural-melee-reflect lane (sibling to the Salamander's
/// Heated Body and the Fire Shield condition-keyed reflect). Resolves
/// the reflected damage through the standard damage pipeline so
/// acid-resistant / -immune attackers shrug it off correctly.
pub static BLACK_PUDDING_CORROSIVE_FORM: MeleeReflect = MeleeReflect {
    damage: ReflectDamage::Dice(Dice::new(1, 8)),
    damage_type: DamageType::Acid,
    label: "corrosive form",
};

/// Black Pudding — CR 4 large ooze. Amorphous tar-like predator from the
/// deep underdark — a featureless blob of corrosive sludge that dissolves
/// armor and weapons on contact. Slots between Gelatinous Cube (CR 2) and
/// Shambling Mound (CR 5) on the formless-horror ladder, the iconic
/// dungeon-cleanup monster that has terrified parties since OD&D.
///
/// Action lanes:
/// - **black pudding pseudopod** — STR-based 1d6+STR bludgeoning melee
///   with a 4d8 acid rider. The acid rider is by far the load-bearing
///   slice; the bludgeoning is a flavor garnish on top of the searing
///   corrosion. Routes through the standard `add_flat_damage_rider`
///   chokepoint so acid-resistant / -immune targets shrug off the
///   rider independently. RAW also corrodes the target's armor on hit
///   (-1 AC, doesn't stack); we omit the armor-degradation clause since
///   the engine doesn't model per-item durability and the headline
///   combat penalty is the acid damage spike.
///
/// Defensive identity: AC 7 (no natural armor — soft amorphous sludge),
/// 68 HP (8d10+24). The signature ooze envelope: **acid immunity** (its
/// own corrosive form is what it eats with), plus immunity to cold,
/// lightning, and slashing damage RAW. We collapse the cold / lightning
/// immunities into the standard damage-modifier map since the engine
/// doesn't yet model the "split when hit by slashing or lightning"
/// reproduction clause — a black pudding cleaved in half RAW becomes two
/// medium puddings with half HP each, which would require a creature-
/// spawn hook the engine doesn't expose. The slashing immunity is
/// surprising on first read (you'd think the blob is *the* slashable
/// monster) but it's RAW: slashing damage just lets the pudding regrow.
/// In this engine the slashing immunity stands in for the "splits
/// instead of dying to slashing" mechanic — a soft de-tune in the
/// pudding's favor, but it preserves the "slashing weapons are bad
/// against this monster" tactical lesson.
///
/// Condition envelope: the standard ooze immunities — Blinded (no eyes
/// to gouge), Charmed (no mind to seduce), Deafened (no ears to ring),
/// Frightened (no fight-or-flight reflex), Prone (no shape to knock
/// over), Asleep (no metabolism to slow). We skip Exhausted /
/// Grappled — a grapple still pins the blob in place via the standard
/// Restrained / Grappled paths.
///
/// **Corrosive Form** (passive, melee retaliation) — RAW: "A creature
/// that touches the pudding or hits it with a melee attack while within
/// 5 feet of it takes 4 (1d8) acid damage." We don't yet wire this
/// retaliation into `MELEE_REFLECT_RIDERS` (it would need a new
/// condition + caster-side install, since the table currently keys on
/// holder conditions like Fire Shield); the load-bearing slice — the
/// pseudopod's 4d8 acid rider — is the headline damage budget. Future
/// polish bucket if a "natural" reflect rider gets a separate lane.
///
/// **Amorphous** (squeeze through 1-inch gaps) — flavor only; the
/// engine's grid is open enough that a Large blob can route around
/// most chokepoints without a per-tile squeeze check.
///
/// Stat shape: AC 7, ~68 HP (8d10+24), STR 16, DEX 5, CON 16, INT 1,
/// WIS 6, CHA 1. Speed 20 (slow ooze ooze). Senses: Blindsight 60
/// (perception via vibration / chemoreception — the pudding has no
/// eyes per RAW). Languages: none. Size Large. CR 4.
pub static BLACK_PUDDING_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&BLACK_PUDDING_PSEUDOPOD);
    CreatureTemplate {
        name: "Black Pudding",
        // 'p' (lowercase) — distinct from existing P (Pixie/Pegasus) and
        // 'b' (Bandit/Boar/Bullywug). 'p' for the formless puddle.
        glyph: 'p',
        ac: 7,
        // 8d10+24 ≈ 68 average per MM (CR 4).
        hitpoints: "8d10+24".parse().unwrap(),
        speed: 20.,
        strength: 16,
        intelligence: 1,
        dexterity: 5,
        wisdom: 6,
        constitution: 16,
        charisma: 1,
        // Ooze envelope: no eyes, sees via vibration. Blindsight 60 with
        // the unstated "blind beyond this radius" clause RAW — we model
        // the sense but don't enforce the cap, matching every other
        // sensed creature in the codebase.
        senses: HashSet::from([SpecialSense::Blindsight(60)]),
        cr: 4.0,
        size: Size::Large,
        creature_type: CreatureType::Ooze,
        actions,
        // Acid immunity (its body IS acid), cold + lightning + slashing
        // immunities per RAW. The slashing immunity stands in for the
        // RAW "splits instead of dying to slashing" mechanic since the
        // engine doesn't model creature-spawn-from-damage hooks.
        damage_modifiers: HashMap::from([
            (DamageType::Acid, DamageModifier::Immunity),
            (DamageType::Cold, DamageModifier::Immunity),
            (DamageType::Lightning, DamageModifier::Immunity),
            (DamageType::Slashing, DamageModifier::Immunity),
        ]),
        // Standard ooze condition envelope — no mind, no eyes, no shape
        // to knock over. Distinct from the gelatinous cube's set in that
        // the black pudding doesn't add Exhausted / Restrained
        // immunities — RAW gives oozes Charmed / Blinded / Deafened /
        // Exhausted / Frightened / Prone but the engine collapses the
        // standard ooze envelope to the six we model uniformly.
        // SRD 5.2 also prints Grappled and Restrained on this row, and
        // they are deliberately left off — the same call the gelatinous
        // cube's envelope makes and for the same reason: an ooze no
        // spell can pin is an ooze no party can control, and Evard's
        // Black Tentacles has to mean something against the thing it
        // most obviously should work on.
        condition_immunities: HashSet::from([
            // SRD 5.2 "…Charmed, Deafened, Exhaustion, Frightened,
            // Grappled, Prone, Restrained". A puddle has nothing to tire.
            Condition::Exhausted,
            Condition::Blinded,
            Condition::Charmed,
            Condition::Deafened,
            Condition::Frightened,
            Condition::Prone,
            Condition::Asleep,
        ]),
        // Corrosive Form: 1d8 acid back at every melee attacker (the
        // signature ooze passive). Wires through the engine's natural-
        // melee-reflect lane in `resolve_attack_outcome`.
        natural_melee_reflect: Some(BLACK_PUDDING_CORROSIVE_FORM),
        ..CreatureTemplate::defaults()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::Coordinate;

    #[test]
    fn black_pudding_template_shape() {
        let a = ActorInstance::from_creature_template(
            &BLACK_PUDDING_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 4.0);
        assert_eq!(a.size(), Size::Large);
        assert_eq!(a.creature_type(), CreatureType::Ooze);
        assert!(a.find_action("black pudding pseudopod").is_some());
    }

    #[test]
    fn black_pudding_has_ooze_damage_envelope() {
        let a = ActorInstance::from_creature_template(
            &BLACK_PUDDING_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        // The signature four immunities — acid (it IS acid), cold,
        // lightning, slashing. Slashing immunity is the "splits, doesn't
        // die" placeholder.
        assert_eq!(
            a.damage_modifier(DamageType::Acid),
            Some(DamageModifier::Immunity)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Cold),
            Some(DamageModifier::Immunity)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Lightning),
            Some(DamageModifier::Immunity)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Slashing),
            Some(DamageModifier::Immunity)
        );
        // Standard ooze condition envelope.
        assert!(a.effectively_immune_to_condition(Condition::Blinded));
        assert!(a.effectively_immune_to_condition(Condition::Charmed));
        assert!(a.effectively_immune_to_condition(Condition::Frightened));
        assert!(a.effectively_immune_to_condition(Condition::Prone));
    }
}
