use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{WEREBOAR_MAUL, WEREBOAR_MULTI, WEREBOAR_TUSKS};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Wereboar — CR 4 medium lycanthrope. The wild-tusker of the
/// lycanthrope family — boar-bodied, tusks for biting, a maul for
/// thumping anything that gets too close. Slots between Werewolf
/// (CR 3) and Werebear (CR 5) on the lycanthrope ladder — the
/// "porcine-flavored" hybrid sitting in the same CR-4 slot as the
/// weretiger but with a sturdier melee profile (heavier dice, no
/// stealth flavor).
///
/// Action lanes:
/// - **wereboar multiattack** — 1 tusks + 1 maul per Action via
///   `CompoundAttack`. Heterogeneous compound (piercing + bludgeoning) —
///   the tusks carry the DC-12 CON-save lycanthropy rider (Poisoned
///   3 rounds as a proxy for RAW's curse), the maul is the steady
///   damage lane.
/// - **wereboar tusks** (standalone) — STR-based 2d6+STR piercing
///   melee with the lycanthropy curse rider. Same DC as the werewolf
///   (12) but a heftier 2d6 die to match the wereboar's CR-4 budget.
/// - **wereboar maul** (standalone) — STR-based 2d6+STR bludgeoning
///   melee. Vanilla `SimpleWeapon`; the chunky-damage lane that pairs
///   with the curse-flavored tusks.
///
/// Defensive identity: AC 10 (no armor — just the boar's thick hide).
/// 78 HP (12d8+24). Non-magical BPS resistance via the shared
/// `damage_modifiers_from` helper — the canonical
/// lycanthrope envelope. RAW also gives the wereboar **Charge** (when
/// it moves 15+ ft and hits with a tusks attack, the target takes an
/// extra 7 (2d6) damage and must succeed on a DC 13 STR save or be
/// knocked prone) — omitted because the engine doesn't track per-turn
/// movement for triggering on-hit bonus damage. The load-bearing kit
/// is the tusks + maul multi, not the charge rider.
///
/// Stat shape: AC 10, ~78 HP (12d8+24), STR 17, DEX 10, CON 15, INT 10,
/// WIS 11, CHA 8. Speed 30. Senses: Darkvision 60. Languages: Common
/// (humanoid form retains speech). Size Medium. CR 4.
pub static WEREBOAR_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*WEREBOAR_MULTI);
    actions.push(&WEREBOAR_TUSKS);
    actions.push(&WEREBOAR_MAUL);
    CreatureTemplate {
        name: "Wereboar",
        // 'p' (lowercase) — distinct from 'w' (Werewolf), 'W' (Wolf),
        // 'B' (Werebear / Brown Bear). The wereboar gets 'p' for
        // "pig" — the boar's silhouette is the read on the map.
        glyph: 'p',
        ac: 10,
        // 12d8+24 ≈ 78 average per MM (CR 4).
        hitpoints: "12d8+24".parse().unwrap(),
        speed: 30.,
        strength: 17,
        intelligence: 10,
        dexterity: 10,
        wisdom: 11,
        constitution: 15,
        charisma: 8,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common]),
        cr: 4.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        // Lycanthrope BPS resistance qualified to nonmagical attacks
        // that aren't silvered — same envelope as Werewolf / Werebear,
        // with RAW's immunity approximated as resistance for the
        // reason the werewolf template gives.
        // RAW: when the wereboar closes at least the clause's distance in a
        // straight line and then connects with its tusks, the hit carries
        // extra 2d6 slashing and a Strength save vs prone. Read at the melee attack
        // chokepoint off `ActorInstance::charge`.
        charge: Some(crate::actions::monster_attacks::WEREBOAR_CHARGE),
        ..CreatureTemplate::resistant_to_nonmagical_nonsilvered_physical()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::{Coordinate, DamageModifier, DamageType};

    #[test]
    fn wereboar_template_shape() {
        let a = ActorInstance::from_creature_template(
            &WEREBOAR_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 4.0);
        assert_eq!(a.size(), Size::Medium);
        assert_eq!(a.creature_type(), CreatureType::Humanoid);
        assert!(a.find_action("wereboar multiattack").is_some());
        assert!(a.find_action("wereboar tusks").is_some());
        assert!(a.find_action("wereboar maul").is_some());
    }

    /// Wereboar Tusks: vanilla swing + DC-12 CON-save lycanthropy rider
    /// (Poisoned 3 rounds on fail). Exercise the end-to-end pipeline
    /// (LycanthropeBite struct → `save_or_condition_rider`) against a
    /// low-CON kobold target so the save reliably fails — confirms the
    /// refactored shared chassis still installs the curse cleanly.
    #[test]
    fn wereboar_tusks_install_lycanthropy() {
        use crate::actions::action_template::Action;
        use crate::actions::monster_attacks::WEREBOAR_TUSKS;
        use crate::actors::creatures::kobolds::KOBOLD_TEMPLATE;
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
        let mut e = EncounterInstance::from_params(&tp, &ap, Some(7)).unwrap();
        let attacker = e
            .instantiate_creature(&WEREBOAR_TEMPLATE, Coordinate::new(5, 5), 1, 0)
            .unwrap();
        let target = e
            .instantiate_creature(&KOBOLD_TEMPLATE, Coordinate::new(6, 5), 0, 0)
            .unwrap();
        // Drive a handful of attacks. At least one is expected to land
        // and trigger the rider; the kobold's CON-9 (-1 mod) reliably
        // fails the DC-12 save when the bite hits.
        let mut got_poison = false;
        for _ in 0..30 {
            for ef in WEREBOAR_TUSKS.side_effects(
                &mut e,
                attacker,
                Some(&vec![target]),
                None,
                None,
            ) {
                ef.apply(&mut e);
            }
            if e.actors[&target].has_condition(Condition::Poisoned) {
                got_poison = true;
                break;
            }
        }
        assert!(
            got_poison,
            "wereboar tusks should occasionally install Poisoned via the lycanthropy save rider"
        );
    }

    #[test]
    fn wereboar_has_lycanthrope_bps_resistance() {
        let a = ActorInstance::from_creature_template(
            &WEREBOAR_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(
            a.nonmagical_damage_modifier(DamageType::Bludgeoning),
            Some(DamageModifier::Resistance)
        );
        assert_eq!(
            a.nonmagical_damage_modifier(DamageType::Piercing),
            Some(DamageModifier::Resistance)
        );
        assert_eq!(
            a.nonmagical_damage_modifier(DamageType::Slashing),
            Some(DamageModifier::Resistance)
        );
        // No magic resistance — the wereboar's defense is HP + BPS half-damage.
        assert!(!a.has_magic_resistance());
    }
}
