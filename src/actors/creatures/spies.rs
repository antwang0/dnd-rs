use crate::actions::class_features::{CUNNING_DASH, CUNNING_DISENGAGE, CUNNING_HIDE};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{SPY_HAND_CROSSBOW, SPY_SHORTSWORD};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size, Skill};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Spy — CR 1 medium humanoid. The one NPC in the appendix whose threat
/// is positional rather than numerical: 27 hit points and a 1d6 blade,
/// paired with **Cunning Action**, which means the spy is never where
/// the swing was aimed.
///
/// Action lanes:
/// - **spy shortsword** — the contact swing, light, so the two-weapon
///   lane is open to it.
/// - **spy hand crossbow** — the reason it opens at range and stays
///   there. Twelve tiles of clean band on a board two dozen wide.
/// - **cunning dash / disengage / hide** — RAW's Cunning Action, all
///   three, as bonus actions. The engine already carries them for the
///   Rogue and they are the same feature, so the spy reads off the same
///   three actions rather than a private copy of them.
///
/// **Sneak Attack (2d6) is deliberately not carried.** It is not a
/// passive flag in this engine: it lives on the rogue-weapon chassis in
/// `class_attacks`, where it scales off the wielder's level, negotiates
/// with Cunning Strike, and consults a positional eligibility rule.
/// Bolting a fixed 2d6 onto an ordinary weapon would be a second
/// implementation of a rule the engine already has one of, and the two
/// would drift. What the spy loses in dice it partly keeps in tempo:
/// Cunning Action is the half of the rogue kit that makes a CR 1 body
/// hard to pin, and it is here in full.
///
/// Stat shape per the SRD NPC appendix: AC 12 (leather), 27 HP (6d8),
/// STR 10 / DEX 15 / CON 10 / INT 12 / WIS 14 / CHA 16. Speed 30.
/// Skills: Deception, Insight, Investigation, Perception, Persuasion,
/// Sleight of Hand, Stealth. CR 1.
pub static SPY_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&SPY_SHORTSWORD);
    actions.push(&SPY_HAND_CROSSBOW);
    actions.push(&CUNNING_DASH);
    actions.push(&CUNNING_DISENGAGE);
    actions.push(&CUNNING_HIDE);
    CreatureTemplate {
        name: "Spy",
        // 'y' (lowercase) — an unclaimed letter, and the tail of the
        // word rather than its head because 's' and 'S' are both deep
        // pools (Skeleton / Shadow / Specter / Spider / Sprite;
        // Salamander / Succubus / Solar).
        glyph: 'y',
        ac: 12,
        // 6d8 ≈ 27 average per the SRD NPC appendix (CR 1).
        hitpoints: "6d8".parse().unwrap(),
        speed: 30.,
        strength: 10,
        dexterity: 15,
        constitution: 10,
        intelligence: 12,
        wisdom: 14,
        charisma: 16,
        skills: HashSet::from([
            Skill::Deception,
            Skill::Insight,
            Skill::Investigation,
            Skill::Perception,
            Skill::Persuasion,
            Skill::SleightOfHand,
            Skill::Stealth,
        ]),
        languages: HashSet::from([Language::Common]),
        cr: 1.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
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
            &SPY_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn spy_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 1.0);
        assert_eq!(a.creature_type(), CreatureType::Humanoid);
        assert!(a.find_action("spy shortsword").is_some());
        assert!(a.find_action("spy hand crossbow").is_some());
    }

    /// Cunning Action is three actions and not one, and a spy that
    /// carried only Dash would still look like it had the feature in
    /// the literal. Naming all three is the only way to say so.
    #[test]
    fn the_spy_carries_all_three_halves_of_cunning_action() {
        let a = make();
        assert!(a.find_action("cunning dash").is_some());
        assert!(a.find_action("cunning disengage").is_some());
        assert!(a.find_action("cunning hide").is_some());
    }
}
