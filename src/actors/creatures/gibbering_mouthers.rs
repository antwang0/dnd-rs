use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    GIBBERING_MOUTHER_BITES, GIBBERING_MOUTHER_BLINDING_SPITTLE,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Gibbering Mouther — CR 2 medium aberration. A writhing mass of mouths
/// and eyes shrieking maddeningly while it chews on whatever it can reach.
/// The shrieking, blinding-spittle, and slow-ground tropes plug into the
/// engine via three load-bearing clauses:
///
/// 1. Bites: a heavy 5d6 + STR melee swing — single attack roll, single
///    target, big die tier for a low-CR creature. RAW prescribes
///    advantage vs prone targets; we collapse to the flat die since the
///    underlying engine handles prone-advantage centrally on attack
///    mode.
/// 2. Blinding Spittle: bonus-action, range 12 (30 ft), 1-tile burst,
///    DEX save vs DC 10 (RAW flat) — failed save Blinds the target until
///    end of next turn. Recharge 5-6 via the standard `("blinding
///    spittle", 5)` entry; the engine's recharge-d6 chokepoint handles
///    refresh at turn-start.
/// 3. Aberrant Ground: not modeled, and not for the reason this note
///    used to give. It once said the trait needed "a per-other-turn
///    aura hook the engine does not have", which is now
///    `engine::emanations` — but the mouther's clause is not one of
///    those. SRD 5.2 reads "the ground in a 10-foot Emanation
///    originating from the mouther is Difficult Terrain", with no save
///    and no condition: it is a *terrain* rule, and the layer it wants
///    is `engine::conjured_terrain`, moving with a creature. The
///    mouther's own 20-foot speed is slow enough that it reads as the
///    swampy-ground creature it is in the meantime.
///
/// Stats roughly track MM Gibbering Mouther at CR 2 — STR 10, DEX 8
/// (the shambling clumsy mass), CON 16, INT 3 (instinct-driven), WIS 10,
/// CHA 6. AC 9 from no natural armor. HP: 7d8+21 = 52 average.
pub static GIBBERING_MOUTHER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GIBBERING_MOUTHER_BITES);
    actions.push(&*GIBBERING_MOUTHER_BLINDING_SPITTLE);
    CreatureTemplate {
        name: "Gibbering Mouther",
        // 'm' (lowercase) — distinct from 'M' (medusa / minotaur uppercase).
        // The amorphous, ground-hugging silhouette evokes the mouther's
        // pulsating mass; lowercase signals "smaller than a humanoid".
        glyph: 'm',
        ac: 9,
        // 7d8+21 = ~52 average per MM.
        hitpoints: "7d8+21".parse().unwrap(),
        // 10 ft RAW — the mouther's "aberrant ground" collapsed to a
        // baked-in slow movement profile.
        speed: 20.,
        strength: 10,
        intelligence: 3,
        dexterity: 8,
        wisdom: 10,
        constitution: 16,
        charisma: 6,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::new(),
        cr: 2.0,
        size: Size::Medium,
        creature_type: CreatureType::Aberration,
        actions,
        // Mouthers brush off the prone condition — they're already a
        // wriggling mass on the ground. Mirrors the Gelatinous Cube /
        // amorphous oozes flavor.
        condition_immunities: HashSet::from([Condition::Prone]),
        // RAW recharge: blinding spittle comes back on a d6 ≥ 5 at the
        // start of each turn. The action's `custom_validate_input` gate
        // and `spend_recharge` consume reads from this entry.
        recharge_abilities: vec![("blinding spittle", 5)],
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
    fn gibbering_mouther_is_a_cr2_aberration() {
        let a = ActorInstance::from_creature_template(
            &GIBBERING_MOUTHER_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 2.0);
        assert!(a.find_action("gibbering bites").is_some());
        assert!(a.find_action("blinding spittle").is_some());
    }

    #[test]
    fn gibbering_mouther_is_prone_immune() {
        let a = ActorInstance::from_creature_template(
            &GIBBERING_MOUTHER_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert!(a.is_immune_to_condition(Condition::Prone));
    }

    #[test]
    fn gibbering_mouther_blinding_spittle_recharges() {
        let a = ActorInstance::from_creature_template(
            &GIBBERING_MOUTHER_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        // Starts available; turn-start recharge logic is centralized in
        // the encounter loop and tested there.
        assert!(a.is_recharge_available("blinding spittle"));
    }
}
