use crate::actions::class_features::{AQUATIC_ONLY_TAG, SWIM_SPEED_TAG, UNDERWATER_BREATHING_TAG};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{GIANT_SEAHORSE_CHARGE, GIANT_SEAHORSE_RAM};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, Skill};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Seahorse — CR 0 tiny beast, and the only creature on the roster with
/// **no attack at all**.
///
/// That is not an omission. SRD 5.2's seahorse has one action, Bubble
/// Dash, which is a movement; it deals no damage, forces no save and
/// targets nobody. The stat block exists to populate a reef, and a reef
/// creature that could hurt somebody would be a different animal.
///
/// It is worth having on the roster for exactly that reason: it is the
/// engine's proof that a combatant with nothing to do is a thing the
/// turn loop, the AI's action picker and the outcome tracker can all
/// survive. Everything else down here has at least a flat 1.
///
/// **Bubble Dash** (RAW: "while underwater, the seahorse moves up to
/// its Swim Speed without provoking Opportunity Attacks") is not
/// modeled as an action — the disengaging half is what `AGILE_TAG`
/// says, and giving a creature that cannot fight a free Disengage
/// would be a rule with nothing to protect. The swim speed it moves at
/// is the half that lands.
///
/// **Water Breathing** collapses to flavor, as it does for every
/// aquatic template here.
///
/// Stat shape: AC 12, ~1 HP (1d4−1, floored at 1), STR 1, DEX 12,
/// CON 8, INT 1, WIS 10, CHA 2. Speed 20 (RAW swim 20). Skills
/// Perception, Stealth. Size Tiny. CR 0. XP 0 per RAW — the only stat
/// block in the document worth literally nothing to kill.
pub static SEAHORSE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    CreatureTemplate {
        name: "Seahorse",
        // 'y' — the curled silhouette; the giant seahorse takes the
        // uppercase 'Y' beside it, the usual size-ladder convention.
        glyph: 'y',
        ac: 12,
        hitpoints: "1d4-1".parse().unwrap(),
        speed: 20.,
        strength: 1,
        intelligence: 1,
        dexterity: 12,
        wisdom: 10,
        constitution: 8,
        charisma: 2,
        skills: HashSet::from([Skill::Perception, Skill::Stealth]),
        cr: 0.0,
        size: Size::Tiny,
        creature_type: CreatureType::Beast,
        // The default roster only — Move, Dodge, Dash and the rest. No
        // weapon is pushed on top, which is the point.
        actions: DEFAULT_ACTIONS.clone(),
        features: HashSet::from([
            SWIM_SPEED_TAG,
            UNDERWATER_BREATHING_TAG,
            // RAW's *only*: a seahorse out of water is a seahorse
            // dying. See `AQUATIC_ONLY_TAG`.
            AQUATIC_ONLY_TAG,
        ]),
        ..CreatureTemplate::defaults()
    }
});

/// Giant Seahorse — CR ½ large beast. The same silhouette at ten times
/// the scale, and unlike its tiny cousin it has something to hit with.
///
/// Action lane:
/// - **giant seahorse ram** — STR-based 2d6+STR bludgeoning, escalated
///   by the charge rider when it comes off a twenty-foot swim.
///
/// RAW writes the escalation as a die swap (2d6 → 2d8) rather than as
/// extra damage, a difference of two points of average; the rider
/// carries `1d4` and no knockdown, which is the closest the charge
/// chassis gets and is documented at `GIANT_SEAHORSE_CHARGE`.
///
/// **Bubble Dash** — RAW's bonus action to swim half speed without
/// provoking — is dropped for the same reason the tiny seahorse's is.
///
/// Stat shape: AC 14, ~16 HP (3d10), STR 15, DEX 12, CON 11, INT 2,
/// WIS 12, CHA 5. Speed 40 (RAW swim 40). Size Large. CR ½. XP 100 per
/// RAW.
pub static GIANT_SEAHORSE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GIANT_SEAHORSE_RAM);
    CreatureTemplate {
        name: "Giant Seahorse",
        glyph: 'Y',
        ac: 14,
        hitpoints: "3d10".parse().unwrap(),
        speed: 40.,
        strength: 15,
        intelligence: 2,
        dexterity: 12,
        wisdom: 12,
        constitution: 11,
        charisma: 5,
        cr: 0.5,
        size: Size::Large,
        creature_type: CreatureType::Beast,
        actions,
        charge: Some(GIANT_SEAHORSE_CHARGE),
        features: HashSet::from([
            SWIM_SPEED_TAG,
            UNDERWATER_BREATHING_TAG,
            // RAW's *only*: a seahorse out of water is a seahorse
            // dying. See `AQUATIC_ONLY_TAG`.
            AQUATIC_ONLY_TAG,
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

    fn make(t: &'static CreatureTemplate) -> ActorInstance {
        ActorInstance::from_creature_template(
            t,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn seahorse_template_shape() {
        let a = make(&SEAHORSE_TEMPLATE);
        assert_eq!(a.cr(), 0.0);
        assert_eq!(a.size(), Size::Tiny);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.has_swim_speed());
    }

    /// The seahorse's defining absence, pinned.
    ///
    /// A future editor reaching for "every creature should be able to
    /// do *something*" would find this the natural template to fix, and
    /// fixing it would be inventing content SRD 5.2 does not have.
    #[test]
    fn the_seahorse_has_nothing_to_attack_with() {
        let a = make(&SEAHORSE_TEMPLATE);
        assert!(
            !a.actions.iter().any(|action| action.deals_damage()),
            "SRD 5.2's seahorse has no attack; giving it one is inventing a stat block"
        );
    }

    #[test]
    fn giant_seahorse_template_shape() {
        let a = make(&GIANT_SEAHORSE_TEMPLATE);
        assert_eq!(a.cr(), 0.5);
        assert_eq!(a.size(), Size::Large);
        assert!(a.find_action("giant seahorse ram").is_some());
        assert!(a.has_swim_speed());
        assert!(a.charge().is_some());
    }
}
