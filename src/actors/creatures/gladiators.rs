use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{GLADIATOR_MULTI, GLADIATOR_SHIELD_BASH, GLADIATOR_SPEAR};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size, Skill};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Gladiator — CR 5 medium humanoid. The appendix's answer to "what if a
/// veteran were a boss". Three attacks a round off a 112-hit-point body
/// behind AC 16, with a Parry reaction taxing the fourth swing that
/// comes back at it.
///
/// Action lanes:
/// - **gladiator multiattack** — 2 spears + 1 shield bash. The bash
///   goes last on purpose; see its declaration for why.
/// - **gladiator spear** (standalone) — 2d6+STR piercing.
/// - **gladiator shield bash** (standalone) — 2d4+STR bludgeoning with
///   a DC 15 STR save vs Prone.
///
/// Defensive identity is two reactions' worth of clause on one flag
/// each. **Parry** (`has_parry`) adds 2 to AC against one melee attack
/// that would otherwise hit, which on a body this size is worth several
/// rounds over a fight. **Brave** (`has_brave`) is advantage on saves
/// against being frightened — the clause that stops a CR 5 melee boss
/// from being answered by a level-1 Cause Fear.
///
/// Stat shape per the SRD NPC appendix: AC 16 (studded leather, shield),
/// 112 HP (15d8+45), STR 18 / DEX 15 / CON 16 / INT 10 / WIS 12 / CHA
/// 15. Speed 30. Proficient STR / DEX / CON saves. Skills: Athletics,
/// Intimidation. CR 5.
pub static GLADIATOR_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*GLADIATOR_MULTI);
    actions.push(&GLADIATOR_SPEAR);
    actions.push(&GLADIATOR_SHIELD_BASH);
    CreatureTemplate {
        name: "Gladiator",
        // 'g' (lowercase) — shared with the low-CR goblinoid band, and
        // the CR gap keeps them apart in the generator's pool.
        glyph: 'g',
        ac: 16,
        // 15d8+45 ≈ 112 average per the SRD NPC appendix (CR 5).
        hitpoints: "15d8+45".parse().unwrap(),
        speed: 30.,
        strength: 18,
        dexterity: 15,
        constitution: 16,
        intelligence: 10,
        wisdom: 12,
        charisma: 15,
        skills: HashSet::from([Skill::Athletics, Skill::Intimidation]),
        languages: HashSet::from([Language::Common]),
        cr: 5.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        proficient_saves: HashSet::from([
            AbilityScoreType::Strength,
            AbilityScoreType::Dexterity,
            AbilityScoreType::Constitution,
        ]),
        has_parry: true,
        has_brave: true,
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
            &GLADIATOR_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn gladiator_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 5.0);
        assert_eq!(a.armor_class(), 16);
        assert!(a.find_action("gladiator multiattack").is_some());
        assert!(a.find_action("gladiator spear").is_some());
        assert!(a.find_action("gladiator shield bash").is_some());
    }

    /// Both defensive clauses, and the three save proficiencies that
    /// make a CR 5 martial hard to shortcut. Asserted together because
    /// each one alone is a stat block somebody could believe was
    /// finished.
    #[test]
    fn the_gladiator_is_hard_to_answer_with_one_spell() {
        let a = make();
        assert!(a.has_parry());
        assert!(a.has_brave());
        assert!(a.is_save_proficient(AbilityScoreType::Strength));
        assert!(a.is_save_proficient(AbilityScoreType::Dexterity));
        assert!(a.is_save_proficient(AbilityScoreType::Constitution));
    }
}
