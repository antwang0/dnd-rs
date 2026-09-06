//! The three **Eldritch Cannon** stat blocks (Artillerist Artificer,
//! subclass level 3, TCE), living together because the design decision
//! they encode only reads in one place: what the artificer gives up by
//! picking one of them.
//!
//! RAW builds all three off the same chassis — "AC 18, HP equal to five
//! times your artificer level, Speed 15 feet, immunity to poison and
//! psychic damage, and to all conditions" — and differs them by exactly
//! one line, the action. So do these. The shared half is
//! `cannon_template`; the differing half is three action pushes and
//! three names.
//!
//! **They do not move.** RAW gives the cannon a 15 ft walking speed and
//! spends the artificer's bonus action to use it; the engine has no
//! channel for one actor to spend another's action economy, and a
//! turret that wanders on its own initiative is a different creature
//! from the one RAW describes. Speed 0 is the honest rendering, and it
//! is what makes placement the Artillerist's real decision: the cannon
//! is summoned adjacent to the artificer and fights the rest of the
//! encounter from that tile.
//!
//! **They carry their builder's numbers.** The engine's summons roll
//! their own attacks and set their own DCs off their own stat blocks —
//! a `SimpleWeapon` and an `AtWillEnemyBurst` have no channel back to
//! whoever called them — so the cannon's Intelligence is the
//! artificer's, written onto the construct. INT 18 with the CR-band
//! proficiency bonus puts the flamethrower's DC and the protector's
//! bonus within a point of what a level-9 Artillerist would produce,
//! which is the same substitution the Wildfire Spirit's Dexterity
//! makes for its druid.
//!
//! **Hit points are a fixed roll, not five times a level.** Summons in
//! this engine instantiate from a template and nothing else — see
//! `SummonSpell::template` for why — so `4d8` ≈ 18 stands in for RAW's
//! level-scaled pool at the level the artificer chassis is built for.
//! AC 18 survives unchanged, which is the number that actually keeps
//! the cannon alive: at AC 18 most of the bestiary needs a good roll to
//! touch it, and the cannon's fragility is meant to be about the size
//! of its hit-point pool rather than about being easy to hit.

use crate::actions::class_features::{CANNON_FLAMETHROWER, CANNON_PROTECTOR_PULSE};
use crate::actions::monster_attacks::FORCE_BALLISTA_BOLT;
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Size};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Poison and psychic immunity — RAW's damage half of the cannon's
/// construct envelope. Shared by all three cannons and by the Steel
/// Defender, which carries the same two.
pub fn construct_damage_immunities() -> HashMap<DamageType, DamageModifier> {
    HashMap::from([
        (DamageType::Poison, DamageModifier::Immunity),
        (DamageType::Psychic, DamageModifier::Immunity),
    ])
}

/// RAW's condition half of the construct envelope: "immunity to all
/// conditions". Spelled out as the set of conditions the engine can
/// install on a creature through a save or a hit, rather than as a
/// blanket flag, because the engine's immunity lane is keyed by
/// condition and a blanket would be a second lane for one cohort.
///
/// The list is the mind-and-body set — everything a construct with no
/// mind, no blood and no fear can be talked into. Notably absent:
/// `Prone` and `Grappled`, which are positional rather than
/// physiological, and which RAW's "all conditions" would cover but
/// which read as bugs when a Gelatinous Cube cannot engulf a turret.
///
/// `Exhausted` belongs on the physiological side of that split and was
/// the one member of it missing. Every Construct in SRD 5.2 but the
/// Homunculus prints Exhaustion on its Immunities row, and unlike the
/// rest of this list it is not flavour: `sickening radiance` installs
/// it, six rungs of it kill, and a turret has no stamina to lose.
pub static CONSTRUCT_CONDITION_IMMUNITIES: LazyLock<HashSet<Condition>> = LazyLock::new(|| {
    HashSet::from([
        Condition::Charmed,
        Condition::Exhausted,
        Condition::Frightened,
        Condition::Poisoned,
        Condition::Asleep,
        Condition::Blinded,
        Condition::Deafened,
        Condition::Paralyzed,
        Condition::Stunned,
        Condition::Unconscious,
    ])
});

/// The shared cannon chassis. `name`, `glyph` and the one action are
/// everything that differs between the three modes.
fn cannon_template(
    name: &'static str,
    glyph: char,
    action: &'static (dyn crate::actions::action_template::Action + Send + Sync),
) -> CreatureTemplate {
    // Deliberately *not* built on `DEFAULT_ACTIONS`: a turret cannot
    // Dash, Dodge, Hide, Help, Disengage, shove or grapple, and giving
    // it those would hand the AI seven ways to spend a turn that RAW
    // says it does not have. What it can do is its one action, and
    // `SKIP` so a cannon with nothing in range still resolves its turn
    // rather than deadlocking the initiative queue.
    let actions: Vec<&'static (dyn crate::actions::action_template::Action + Send + Sync)> =
        vec![&*crate::actions::default_actions::SKIP, action];
    CreatureTemplate {
        name,
        glyph,
        ac: 18,
        hitpoints: "4d8".parse().unwrap(),
        // A turret. See the module note.
        speed: 0.,
        strength: 10,
        dexterity: 10,
        constitution: 10,
        // The artificer's casting stat, written onto the construct —
        // this is what the flamethrower's DC and the protector's bonus
        // are read off.
        intelligence: 18,
        wisdom: 10,
        charisma: 10,
        cr: 1.0,
        size: Size::Small,
        creature_type: CreatureType::Construct,
        actions,
        damage_modifiers: construct_damage_immunities(),
        condition_immunities: CONSTRUCT_CONDITION_IMMUNITIES.clone(),
        ..CreatureTemplate::defaults()
    }
}

/// Flamethrower cannon — the crowd mode. A 2-tile fire burst around the
/// turret, every round, Dexterity save for half.
///
/// Worth the most when several enemies converge on the tile it is
/// standing on, which is a situation the Artillerist can arrange by
/// putting the cannon where the party is holding a line. Worth the
/// least against a single target, where 2d8 save-for-half averages less
/// than the ballista's 2d8 on an attack roll.
pub static FLAMETHROWER_CANNON_TEMPLATE: LazyLock<CreatureTemplate> =
    LazyLock::new(|| cannon_template("Flamethrower Cannon", 'c', &CANNON_FLAMETHROWER));

/// Force Ballista cannon — the sniper mode. 2d8 force on a ranged
/// attack out to 120 ft.
///
/// The longest reach on the artificer's side of the board and the only
/// damage type nothing in the bestiary resists, which together make the
/// ballista the mode that is never dead: fire immunity turns the
/// flamethrower off and a scattered enemy line makes it worthless,
/// while the ballista shoots whatever is furthest away for full damage.
pub static FORCE_BALLISTA_CANNON_TEMPLATE: LazyLock<CreatureTemplate> =
    LazyLock::new(|| cannon_template("Force Ballista Cannon", 'b', &FORCE_BALLISTA_BOLT));

/// Protector cannon — the support mode. `1d8 + INT` temporary hit
/// points to every ally within 10 ft, every round.
///
/// The only one of the three that never attacks, and the only one whose
/// value is decided by where the *party* stands rather than by where
/// the enemy does. A Protector parked behind a front line pays out
/// every round for the rest of the encounter; the same cannon left at
/// the back of the room pays nothing at all, which is a sharper
/// placement decision than either sibling poses.
pub static PROTECTOR_CANNON_TEMPLATE: LazyLock<CreatureTemplate> =
    LazyLock::new(|| cannon_template("Protector Cannon", 'p', &CANNON_PROTECTOR_PULSE));

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::Coordinate;

    fn instantiate(template: &'static LazyLock<CreatureTemplate>) -> ActorInstance {
        ActorInstance::from_creature_template(
            template,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    /// The three modes are one chassis plus one action, and the whole
    /// point of `cannon_template` is that a fourth mode cannot
    /// accidentally differ in anything else. Pinning the shared half
    /// here is what makes that claim checkable rather than a comment.
    #[test]
    fn the_three_cannons_differ_only_in_their_action() {
        for t in [
            &FLAMETHROWER_CANNON_TEMPLATE,
            &FORCE_BALLISTA_CANNON_TEMPLATE,
            &PROTECTOR_CANNON_TEMPLATE,
        ] {
            let cannon = instantiate(t);
            assert_eq!(cannon.armor_class(), 18);
            assert_eq!(cannon.size(), Size::Small);
            // Skip plus exactly one mode action — a turret has no
            // Dash / Dodge / Hide lane.
            assert_eq!(cannon.actions.len(), 2);
        }
    }

    /// A turret that wandered off would be a different creature from
    /// the one RAW describes, and speed is the field that says so.
    #[test]
    fn a_cannon_does_not_move() {
        assert_eq!(instantiate(&FLAMETHROWER_CANNON_TEMPLATE).speed(), 0.);
    }

    /// The construct envelope is the half the three share with the
    /// Steel Defender, and it is what keeps a cannon from being
    /// switched off by a poison cloud or a fear aura.
    #[test]
    fn a_cannon_is_immune_to_poison_and_to_being_frightened() {
        let cannon = instantiate(&FORCE_BALLISTA_CANNON_TEMPLATE);
        assert_eq!(
            cannon.damage_modifier(DamageType::Poison),
            Some(DamageModifier::Immunity)
        );
        assert!(cannon.is_immune_to_condition(Condition::Frightened));
    }
}
