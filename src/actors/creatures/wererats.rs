use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{WERERAT_BITE, WERERAT_MULTI, WERERAT_SHORTSWORD};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size, Skill, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Wererat — CR 2 medium lycanthrope. The skulking sewer-dweller of
/// the lycanthrope family — rat-bodied in hybrid form, switching
/// between bite + shortsword in melee. Slots at the bottom of the
/// lycanthrope ladder (CR 2) — the smallest and sneakiest of the
/// wereXX cohort, distinct from the brute wereboar/werebear pair by
/// its DEX-finesse shortsword lane.
///
/// Action lanes:
/// - **wererat multiattack** — 1 bite + 1 shortsword per Action via
///   `CompoundAttack`. Heterogeneous compound — both piercing, but
///   the shortsword routes through DEX (the wererat's signature
///   finesse-weapon flavor). The bite carries the DC-11 CON-save
///   lycanthropy rider (Poisoned 3 rounds, proxy for RAW's curse).
/// - **wererat bite** (standalone) — STR-based 1d4+STR piercing
///   melee with the lycanthropy curse rider. Lowest DC of the wereXX
///   family (11) — wererat is the weakest curse-carrier per RAW.
/// - **wererat shortsword** (standalone) — DEX-based 1d6+DEX piercing
///   melee. Vanilla `SimpleWeapon`; the finesse-flavored secondary swing.
///
/// Defensive identity: AC 12 (light armor + DEX). 33 HP (6d8+6). Non-
/// magical BPS resistance via the shared `damage_modifiers_from`
/// helper — the canonical lycanthrope envelope. RAW gives the wererat
/// **Keen Smell** (advantage on Perception checks using smell) — omitted
/// because the engine doesn't tag Perception by sense channel.
///
/// Stat shape: AC 12, ~33 HP (6d8+6), STR 10, DEX 15, CON 12, INT 11,
/// WIS 10, CHA 8. Speed 30. Senses: Darkvision 60. Languages: Common
/// (humanoid form retains speech). Size Medium. CR 2.
pub static WERERAT_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*WERERAT_MULTI);
    actions.push(&WERERAT_BITE);
    actions.push(&WERERAT_SHORTSWORD);
    CreatureTemplate {
        name: "Wererat",
        // 'r' (lowercase) — distinct from 'R' (Roper / Wraith uses R),
        // 'w' (Werewolf), 'B' (Werebear). The wererat gets 'r' for
        // "rat" — the rodent silhouette is the read on the map.
        glyph: 'r',
        ac: 12,
        // 6d8+6 ≈ 33 average per MM (CR 2).
        hitpoints: "6d8+6".parse().unwrap(),
        speed: 30.,
        strength: 10,
        intelligence: 11,
        dexterity: 15,
        wisdom: 10,
        constitution: 12,
        charisma: 8,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common]),
        cr: 2.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        skills: HashSet::from([Skill::Perception, Skill::Stealth]),
        ..CreatureTemplate::resistant_to_nonmagical_physical()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::{Coordinate, DamageModifier, DamageType};

    #[test]
    fn wererat_template_shape() {
        let a = ActorInstance::from_creature_template(
            &WERERAT_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 2.0);
        assert_eq!(a.size(), Size::Medium);
        assert_eq!(a.creature_type(), CreatureType::Humanoid);
        assert!(a.find_action("wererat multiattack").is_some());
        assert!(a.find_action("wererat bite").is_some());
        assert!(a.find_action("wererat shortsword").is_some());
    }

    #[test]
    fn wererat_has_lycanthrope_bps_resistance() {
        let a = ActorInstance::from_creature_template(
            &WERERAT_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(
            a.nonmagical_damage_modifier(DamageType::Piercing),
            Some(DamageModifier::Resistance)
        );
        assert!(!a.has_magic_resistance());
    }
}
