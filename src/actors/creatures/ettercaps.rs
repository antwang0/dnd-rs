use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{ETTERCAP_BITE, ETTERCAP_CLAWS, ETTERCAP_MULTI, ETTERCAP_WEB};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, Skill, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Ettercap — CR 2 medium monstrosity. The spider-humanoid trapper of
/// the underdark — bite + claws in melee, a 30-ft web shot for the
/// ranged restraint lane. Slots next to Sea Hag (CR 2), Polar Bear
/// (CR 2), and Carrion Crawler (CR 2) on the mid-low monstrosity
/// bench; the only template in the pool that combines a Restrained
/// rider on a save-only ranged action with a venom-flavored bite.
///
/// Action lanes:
/// - **ettercap multiattack** — 1 bite + 1 claws per Action via
///   `CompoundAttack`. Heterogeneous compound (piercing + slashing) —
///   the bite carries the DC-11 CON-save venom rider (extra 2d4 poison
///   + Poisoned 2 rounds on fail), the claws are the steady damage lane.
/// - **ettercap bite** (standalone) — STR-based 1d8+STR piercing
///   melee with the venom rider. Mirrors the `SpiderBite` shape but
///   tuned to the ettercap's CR-2 budget (1d8 base vs spider's 1d10).
/// - **ettercap claws** (standalone) — STR-based 2d4+STR slashing
///   melee. Vanilla `SimpleWeapon`; the steady damage lane.
/// - **ettercap web** (standalone) — DEX save vs Restrained at 30 ft,
///   no attack roll. The ranged-control lane: lock down a PC at
///   distance, then close to bite. Restrained-immune targets short-
///   circuit before the save (the spider's body-control kit doesn't
///   apply to the incorporeal / gaseous envelope).
///
/// Defensive identity: AC 13 (natural armor — chitinous carapace).
/// 44 HP (8d8+8). No damage resistances or immunities — ettercap is a
/// straightforward CR-2 melee+control creature, vulnerable to focused
/// fire. Spider Climb (RAW: can walk on walls and ceilings) omitted —
/// the engine doesn't model 3D vertical movement on a 2D grid; the
/// load-bearing kit is the web + bite combo, not vertical positioning.
/// Web Sense / Web Walker (RAW: ignores movement restrictions caused
/// by webbing) omitted for the same reason — the engine doesn't track
/// per-tile web overlays.
///
/// Stat shape: AC 13, ~44 HP (8d8+8), STR 14, DEX 15, CON 13, INT 7,
/// WIS 12, CHA 8. Speed 30. Senses: Darkvision 60. No languages (RAW
/// gives Ettercap as understanding Common but not speaking — we drop
/// the comprehension flag since the engine doesn't surface listener-
/// only languages). Size Medium. CR 2.
pub static ETTERCAP_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*ETTERCAP_MULTI);
    actions.push(&ETTERCAP_BITE);
    actions.push(&ETTERCAP_CLAWS);
    actions.push(&*ETTERCAP_WEB);
    CreatureTemplate {
        name: "Ettercap",
        // 'E' (uppercase) — distinct from 'e' (used by elementals etc.),
        // 'Erinyes' / 'Ettin' both already have other glyphs. The
        // ettercap is the only spider-humanoid in the pool so 'E' is
        // a clean read.
        glyph: 'E',
        ac: 13,
        // 8d8+8 ≈ 44 average per MM (CR 2).
        hitpoints: "8d8+8".parse().unwrap(),
        speed: 30.,
        strength: 14,
        intelligence: 7,
        dexterity: 15,
        wisdom: 12,
        constitution: 13,
        charisma: 8,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::new(),
        cr: 2.0,
        size: Size::Medium,
        creature_type: CreatureType::Monstrosity,
        actions,
        // Web is Recharge 5–6 per RAW — gated on the shared recharge
        // chassis (start-of-turn d6 roll, available again on 5+).
        // Keeps the ettercap from spamming the web every turn.
        recharge_abilities: vec![("ettercap_web", 5)],
        skills: HashSet::from([Skill::Perception, Skill::Stealth]),
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
    fn ettercap_template_shape() {
        let a = ActorInstance::from_creature_template(
            &ETTERCAP_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 2.0);
        assert_eq!(a.size(), Size::Medium);
        assert_eq!(a.creature_type(), CreatureType::Monstrosity);
        // The ettercap's four action lanes — multi (bite + claws)
        // primary, bite + claws standalone, plus the ranged web restraint.
        assert!(a.find_action("ettercap multiattack").is_some());
        assert!(a.find_action("ettercap bite").is_some());
        assert!(a.find_action("ettercap claws").is_some());
        assert!(a.find_action("ettercap web").is_some());
    }

    /// Ettercap Web: ranged DEX save vs Restrained on fail. Drive
    /// several casts against a low-DEX target so the save reliably
    /// fails — confirms the action installs Restrained via the
    /// `save_or_condition_rider` chassis.
    #[test]
    fn ettercap_web_restrains_low_dex_target() {
        use crate::actions::action_template::Action;
        use crate::actions::monster_attacks::ETTERCAP_WEB;
        use crate::actors::creatures::ogres::OGRE_TEMPLATE;
        use crate::conditions::Condition;
        use crate::engine::encounter::EncounterInstance;
        use crate::engine::actor_gen::ActorGenParams;
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
        let mut e = EncounterInstance::from_params(&tp, &ap, Some(11)).unwrap();
        let attacker = e
            .instantiate_creature(&ETTERCAP_TEMPLATE, Coordinate::new(5, 5), 1, 0)
            .unwrap();
        // Ogre DEX 8 (-1 mod) reliably fails the DC-11 web save.
        let target = e
            .instantiate_creature(&OGRE_TEMPLATE, Coordinate::new(10, 5), 0, 0)
            .unwrap();
        let mut got_restrained = false;
        for _ in 0..30 {
            for ef in ETTERCAP_WEB.side_effects(
                &mut e,
                attacker,
                Some(&vec![target]),
                None,
                None,
            ) {
                ef.apply(&mut e);
            }
            if e.actors[&target].has_condition(Condition::Restrained) {
                got_restrained = true;
                break;
            }
            // Strip Restrained between iterations so we exercise a fresh
            // save each loop without accumulating timers.
            e.actors.get_mut(&target).unwrap().remove_condition(Condition::Restrained);
        }
        assert!(
            got_restrained,
            "ettercap web should occasionally restrain a low-DEX ogre via the DC-11 DEX save"
        );
    }

    /// Ettercap Web is Recharge 5–6. Verify the validator gates the
    /// action on the recharge resource — fresh ettercap can fire it,
    /// post-use it's locked out until the start-of-turn d6 lands ≥ 5.
    #[test]
    fn ettercap_web_recharge_gating() {
        use crate::actions::action_template::Action;
        use crate::actions::monster_attacks::ETTERCAP_WEB;
        use crate::engine::encounter::EncounterInstance;
        use crate::engine::actor_gen::ActorGenParams;
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
        let mut e = EncounterInstance::from_params(&tp, &ap, Some(13)).unwrap();
        let attacker = e
            .instantiate_creature(&ETTERCAP_TEMPLATE, Coordinate::new(5, 5), 1, 0)
            .unwrap();
        let target = e
            .instantiate_creature(&ETTERCAP_TEMPLATE, Coordinate::new(10, 5), 0, 0)
            .unwrap();
        // Fresh ettercap: web is available.
        assert!(e.actors[&attacker].is_recharge_available("ettercap_web"));
        assert!(ETTERCAP_WEB.validate_input(
            &e,
            attacker,
            Some(&vec![target]),
            None,
            None
        ));
        // Spend the web — recharge flips off.
        for ef in ETTERCAP_WEB.side_effects(
            &mut e,
            attacker,
            Some(&vec![target]),
            None,
            None,
        ) {
            ef.apply(&mut e);
        }
        assert!(!e.actors[&attacker].is_recharge_available("ettercap_web"));
        assert!(!ETTERCAP_WEB.validate_input(
            &e,
            attacker,
            Some(&vec![target]),
            None,
            None
        ));
    }

    #[test]
    fn ettercap_has_no_immunities() {
        let a = ActorInstance::from_creature_template(
            &ETTERCAP_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        // No magic resistance — ettercap is a straightforward CR-2
        // melee+control creature.
        assert!(!a.has_magic_resistance());
    }
}
