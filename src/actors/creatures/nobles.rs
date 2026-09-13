use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::NOBLE_RAPIER;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size, Skill};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Noble — CR ⅛ medium humanoid. A rapier, plate-adjacent AC, and a
/// **Parry** reaction that is worth more than either.
///
/// RAW: "Parry. The noble adds 2 to its AC against one melee attack
/// that would hit it. To do so, the noble must see the attacker and be
/// wielding a melee weapon." Carried on `parry_bonus`, the field the
/// whole parry list reads, so the noble's reaction competes for the
/// same slot everything else does — and the sight clause comes free
/// with the shared `reactive_reducer_eligible` gate.
///
/// It used to ride `has_parry`, which is the Battle Master *maneuver*
/// of that name: a damage clamp gated on a superiority die, not an AC
/// bump, and gated on a die the noble does not have — so the one good
/// trick on this stat block did nothing at all.
///
/// The reaction is why a CR ⅛ body sits on AC 15 and does not simply
/// die. Nine hit points is one hit from anything, and Parry is the
/// clause that makes which hit it is a question rather than a formality.
///
/// Stat shape per the SRD NPC appendix: AC 15 (breastplate), 9 HP
/// (2d8), STR 11 / DEX 12 / CON 11 / INT 12 / WIS 14 / CHA 16. Speed
/// 30. Skills: Deception, Insight, Persuasion. CR ⅛.
pub static NOBLE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&NOBLE_RAPIER);
    CreatureTemplate {
        name: "Noble",
        // 'n' (lowercase) — free in the humanoid band. 'N' is the
        // Nightmare / Nothic / Nalfeshnee pool.
        glyph: 'n',
        ac: 15,
        // 2d8 ≈ 9 average per the SRD NPC appendix (CR ⅛).
        hitpoints: "2d8".parse().unwrap(),
        speed: 30.,
        strength: 11,
        dexterity: 12,
        constitution: 11,
        intelligence: 12,
        wisdom: 14,
        charisma: 16,
        skills: HashSet::from([Skill::Deception, Skill::Insight, Skill::Persuasion]),
        languages: HashSet::from([Language::Common]),
        cr: 0.125,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        // SRD 5.2 **Parry** (Reaction): *"the noble adds 2 to its AC
        // against that attack."* A CR 1/8 stat block with one good
        // trick, which is what fencing lessons buy.
        parry_bonus: 2,
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
            &NOBLE_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn noble_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.125);
        assert_eq!(a.armor_class(), 15);
        assert!(a.find_action("noble rapier").is_some());
    }

    /// Parry needs a melee weapon in hand to be legal RAW, so the flag
    /// and the rapier are one claim rather than two: a noble with the
    /// reaction and nothing to parry with would be a stat block with a
    /// dead line on it.
    #[test]
    fn the_noble_parries_with_something_it_is_actually_holding() {
        let a = make();
        assert_eq!(a.parry_bonus(), 2, "SRD 5.2 prints +2");
        assert!(a.find_action("noble rapier").is_some());
    }
}
