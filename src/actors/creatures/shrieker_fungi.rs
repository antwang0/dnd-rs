use crate::actions::class_features::SHRIEKER_TAG;
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Shrieker Fungus — CR 0 medium plant, and the only creature in this
/// bestiary that cannot attack anything.
///
/// SRD 5.2's whole stat block is a body and one reaction:
///
/// > **Shriek.** *Trigger:* A creature or a source of Bright Light moves
/// > within 30 feet of the shrieker. *Response:* The shrieker emits a
/// > shriek audible within 300 feet of itself for 1 minute or until the
/// > shrieker dies.
///
/// No attack, no damage, no save. It is an alarm with hit points, and
/// the last SRD 5.2 monster this bestiary was missing.
///
/// ## What the shriek does here
///
/// RAW's answer is "it makes a noise", and at a table the consequence is
/// the DM rolling for what the noise brings. This engine has no
/// wandering monsters to bring, so a literal reading would have made the
/// reaction a log line — which is a stat block that does nothing, and
/// this file's standing policy for a clause with no combat surface is to
/// leave it out rather than to ship an inert flag.
///
/// The one consequence the engine *can* express is the one the shriek is
/// for: it wakes the room. `Condition::Surprised` is the engine's word
/// for a creature that has been caught unaware, and it is worth real
/// mechanics — an assassin scores an automatic critical hit against a
/// surprised target, and every attacker has advantage on one. A
/// screaming fungus is the exact reason a party's ambush fails, so the
/// shriek strips `Surprised` from every creature that can hear it, which
/// on any board this engine generates is everyone.
///
/// That is a divergence and is named as one: RAW attaches no such clause.
/// It is the reading that makes the creature *be* what its stat block
/// describes rather than a plant with thirteen hit points, and it points
/// the way RAW points — the shrieker's whole purpose is that sneaking
/// past it does not work.
///
/// See `EncounterInstance::dispatch_shrieks` for the trigger.
///
/// ## The rest of the block
///
/// Stat shape per the SRD: AC 5, 13 HP (3d8), speed 5, STR 1 / DEX 1 /
/// CON 10 / INT 1 / WIS 3 / CHA 1, Blindsight 30, CR 0. The condition
/// immunities are the Violet Fungus's exactly — Blinded, Charmed,
/// Deafened, Frightened — and for the same reasons: no eyes, no mind,
/// and RAW lists them. Exhaustion is deliberately absent, matching the
/// SRD's own per-stat-block treatment of the Plant type.
///
/// Poison immunity is *not* here. The Violet Fungus carries it and this
/// one does not, because SRD 5.2 prints it on the one and not the other
/// — a difference that reads like an oversight in the book and is
/// reproduced anyway, on the same principle every other row of the
/// bestiary follows.
pub static SHRIEKER_FUNGUS_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Nothing but the default action set. The shrieker has no attack in
    // SRD 5.2 and is given none here — it moves five feet a round, it
    // screams once, and it dies. A hand-written swing to "make it a
    // monster" would be a monster this book does not have.
    let actions = DEFAULT_ACTIONS.clone();
    CreatureTemplate {
        name: "Shrieker Fungus",
        // 'F' — the fungal / plant band it shares with the Violet
        // Fungus, which is the right neighbour: on a map the two are
        // the same thing until one of them touches you.
        glyph: 'F',
        ac: 5,
        // 3d8 ≈ 13 average per the SRD.
        hitpoints: "3d8".parse().unwrap(),
        speed: 5.,
        strength: 1,
        dexterity: 1,
        constitution: 10,
        intelligence: 1,
        wisdom: 3,
        charisma: 1,
        senses: HashSet::from([SpecialSense::Blindsight(30)]),
        cr: 0.0,
        size: Size::Medium,
        creature_type: CreatureType::Plant,
        actions,
        // The one thing it does. Not an action on the list — RAW makes
        // it a reaction with a trigger nothing on this board declares,
        // so it rides `EncounterInstance::dispatch_shrieks` off the
        // movement event instead, gated on this tag.
        features: HashSet::from([SHRIEKER_TAG]),
        condition_immunities: HashSet::from([
            Condition::Blinded,
            Condition::Charmed,
            Condition::Deafened,
            Condition::Frightened,
        ]),
        ..CreatureTemplate::defaults()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::Coordinate;

    fn make() -> ActorInstance {
        ActorInstance::from_creature_template(
            &SHRIEKER_FUNGUS_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn shrieker_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.0);
        assert_eq!(a.creature_type(), CreatureType::Plant);
        assert!(a.effectively_immune_to_condition(Condition::Frightened));
        assert!(a.effectively_immune_to_condition(Condition::Blinded));
    }

    /// The shrieker swings at nothing, and that is the stat block rather
    /// than an omission.
    ///
    /// Pinned because it is the only creature in the bestiary of which
    /// it is true, so every sweep that assumes "a monster has an attack"
    /// meets its counterexample here first — and because the tempting
    /// fix for a monster that does nothing is to give it something,
    /// which SRD 5.2 pointedly does not.
    #[test]
    fn the_shrieker_carries_no_attack_at_all() {
        let a = make();
        let armed: Vec<&str> = a
            .actions
            .iter()
            .filter(|act| act.is_harmful() && act.deals_damage())
            .map(|act| act.name())
            .collect();
        assert!(
            armed.is_empty(),
            "the shrieker has grown teeth it has no stat block for: {armed:?}"
        );
    }
}
