use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{BLINK_DOG_BITE, BLINK_DOG_TELEPORT};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Blink Dog — CR ¼ medium fey. The friendly teleporting hound of the
/// Feywild — looks like a slim wolf but phases in and out of reality at
/// will. Slots at the bottom of the fey ladder alongside Pixie / Sprite
/// (CR ¼) and below Dryad (CR 1) — the canid-mobility variant of the
/// fey trickster archetype.
///
/// Action lanes:
/// - **blink dog bite** — STR-based 1d6+STR piercing melee. The
///   damage-only primary lane. Vanilla `SimpleWeapon::melee` — no
///   per-hit rider; the load-bearing combat clause is the teleport.
/// - **blink** — bonus-action 40 ft teleport, Recharge 4–6. The
///   signature mobility tool: the dog phases out of melee after a
///   bite, repositions, then phases back in to bite again next turn.
///   The teleport is range-16-tile (40 ft on this 2.5 ft grid) and
///   LOS-gated (the dog needs to see its destination). Recharge 4
///   tempo means roughly every other turn the teleport is back online.
///
/// Defensive identity: no resistances or immunities — the blink dog is
/// a beast-tier silhouette in everything but type. The 22-HP statline
/// plus the AC 13 reads as a fragile scout that survives via mobility,
/// not durability. No magic resistance (it's a fey, not a caster). No
/// fey-ancestry immunity bundle either — RAW gives blink dogs neither
/// Magic Resistance nor Fey Ancestry. (Pixie / Sprite at the same CR
/// are the magical fey; the blink dog is the canid-mobility variant.)
///
/// Stat shape: AC 13 (the slim wolf's natural armor), 22 HP (4d8+4),
/// STR 12, DEX 17 (the canid burst), CON 13, INT 10, WIS 13, CHA 11.
/// Speed 40 (the canid burst speed, matching dire wolves). Senses:
/// Darkvision 60. Languages: Blink Dog (RAW — its own racial tongue,
/// understands Sylvan but can't speak it). We surface only Sylvan
/// since the engine doesn't track a "Blink Dog" language. Size Medium.
/// CR ¼.
pub static BLINK_DOG_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&BLINK_DOG_BITE);
    actions.push(&*BLINK_DOG_TELEPORT);
    CreatureTemplate {
        name: "Blink Dog",
        // 'k' (lowercase) — distinct from 'd' (Dretch / Dire Wolf), 'D'
        // (Death Dog), and the wolf cohort. The 'k' reads as the
        // canine silhouette of the Feywild and stays distinct from the
        // mundane-canid pool.
        glyph: 'k',
        ac: 13,
        // 4d8+4 ≈ 22 average per MM (CR ¼).
        hitpoints: "4d8+4".parse().unwrap(),
        speed: 40.,
        strength: 12,
        intelligence: 10,
        dexterity: 17,
        wisdom: 13,
        constitution: 13,
        charisma: 11,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Sylvan]),
        cr: 0.25,
        size: Size::Medium,
        creature_type: CreatureType::Fey,
        actions,
        // Teleport is Recharge 4–6 per RAW (the dog phases back in after
        // a brief cooldown). Gated on the shared recharge chassis —
        // start-of-turn d6 roll, available again on 4+. Keeps the
        // blink-bite tempo from being a free repositioning loop.
        recharge_abilities: vec![("blink_dog_teleport", 4)],
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
    fn blink_dog_template_shape() {
        let a = ActorInstance::from_creature_template(
            &BLINK_DOG_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 0.25);
        assert_eq!(a.size(), Size::Medium);
        assert_eq!(a.creature_type(), CreatureType::Fey);
        assert!(a.find_action("blink dog bite").is_some());
        assert!(a.find_action("blink").is_some());
    }

    #[test]
    fn blink_dog_has_no_magic_envelope() {
        let a = ActorInstance::from_creature_template(
            &BLINK_DOG_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        // No magic resistance, no fey-ancestry immunity bundle — the
        // blink dog is the canid-mobility variant of the fey pool,
        // not the caster-fey lane (Pixie / Sprite).
        assert!(!a.has_magic_resistance());
    }

    /// Blink Dog Teleport is Recharge 4-6. Verify the validator gates the
    /// action on the recharge resource — fresh blink dog can fire it,
    /// post-use it's locked out until the start-of-turn d6 lands ≥ 4.
    #[test]
    fn blink_dog_teleport_recharge_gating() {
        use crate::actions::action_template::Action;
        use crate::engine::actor_gen::ActorGenParams;
        use crate::engine::encounter::EncounterInstance;
        use crate::engine::terrain_gen::TerrainGenParams;
        let tp = TerrainGenParams {
            width: 30,
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
        let mut e = EncounterInstance::from_params(&tp, &ap, Some(17)).unwrap();
        let attacker = e
            .instantiate_creature(&BLINK_DOG_TEMPLATE, Coordinate::new(5, 5), 1, 0)
            .unwrap();
        let dest = vec![Coordinate::new(8, 5)];
        // Fresh blink dog: teleport is available against a known-good
        // (in-bounds, unoccupied) destination tile.
        assert!(e.actors[&attacker].is_recharge_available("blink_dog_teleport"));
        assert!(BLINK_DOG_TELEPORT.validate_input(&e, attacker, None, Some(&dest), None));
        // Spend the teleport — recharge flips off, and the actor moved.
        for ef in BLINK_DOG_TELEPORT.side_effects(&mut e, attacker, None, Some(&dest), None) {
            ef.apply(&mut e);
        }
        assert!(!e.actors[&attacker].is_recharge_available("blink_dog_teleport"));
        assert!(!BLINK_DOG_TELEPORT.validate_input(&e, attacker, None, Some(&dest), None));
        assert_eq!(e.actors[&attacker].location(), Coordinate::new(8, 5));
    }
}
