use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::BADGER_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Size, Skill, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Badger — CR 0 tiny beast. The odd one on the CR-0 shelf: everything
/// else down here is made of paper, and the badger has Constitution 16
/// and **resistance to poison**. It is the ambient animal that survives
/// the stinking cloud somebody dropped on the whole encounter.
///
/// Action lane:
/// - **badger bite** — flat 1 piercing on a STR-based roll. A `1d1`
///   die rather than a literal so a crit doubles it through the shared
///   crit chassis; the badger will not be killing anything either way.
///
/// **Burrow 5** is not modeled — the engine's board has no third
/// dimension for a burrower to use, and five feet of it would not
/// change a turn even if it did.
///
/// Defensive identity: AC 11, ~5 HP (1d4+3), poison resistance, and
/// darkvision 30. The resistance is the only combat-relevant line, and
/// it is the reason to reach for a badger over a rat.
///
/// Stat shape: AC 11, ~5 HP, STR 10, DEX 11, CON 16, INT 2, WIS 12,
/// CHA 5. Speed 20. Skills Perception. Senses Darkvision 30. Size Tiny.
/// CR 0. XP 10 per RAW.
pub static BADGER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&BADGER_BITE);
    CreatureTemplate {
        name: "Badger",
        // 'b' — the low-slung burrower, beside the boar's own 'b' in
        // the lowercase ground-mammal cohort.
        glyph: 'b',
        ac: 11,
        hitpoints: "1d4+3".parse().unwrap(),
        speed: 20.,
        strength: 10,
        intelligence: 2,
        dexterity: 11,
        wisdom: 12,
        constitution: 16,
        charisma: 5,
        skills: HashSet::from([Skill::Perception]),
        senses: HashSet::from([SpecialSense::Darkvision(30)]),
        cr: 0.0,
        size: Size::Tiny,
        creature_type: CreatureType::Beast,
        actions,
        // RAW **Resistances: Poison** — the one defensive line on the
        // CR-0 shelf that a spell can actually run into.
        damage_modifiers: HashMap::from([(DamageType::Poison, DamageModifier::Resistance)]),
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
            &BADGER_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn badger_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.0);
        assert_eq!(a.size(), Size::Tiny);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("badger bite").is_some());
    }

    /// The poison resistance is the badger's only combat-relevant
    /// line — pin it so a template refactor can't quietly turn the
    /// badger into a rat with a bigger Constitution.
    #[test]
    fn badger_shrugs_off_poison() {
        let a = make();
        assert_eq!(
            a.damage_modifier(DamageType::Poison),
            Some(DamageModifier::Resistance)
        );
    }
}
