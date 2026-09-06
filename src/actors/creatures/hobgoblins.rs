use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    HOBGOBLIN_CAPTAIN_GREATSWORD, HOBGOBLIN_CAPTAIN_LONGBOW, HOBGOBLIN_CAPTAIN_MULTI, LONGBOW,
    SCIMITAR,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Hobgoblin — CR 1/2 martial humanoid. Disciplined and well-armored
/// (chain mail + shield → AC 18) compared to the rabble goblin. Carries
/// both a scimitar (melee) and a longbow (ranged) so it can pivot to
/// whichever range suits the moment. No special features — the threat
/// is just having tankier mooks at the same XP price as bandits.
pub static HOBGOBLIN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&SCIMITAR);
    actions.push(&LONGBOW);
    CreatureTemplate {
        name: "Hobgoblin",
        // 'H' is unused — keep the glyph mnemonic for hobgoblin.
        glyph: 'H',
        ac: 18,
        hitpoints: "2d8+2".parse().unwrap(),
        strength: 13,
        dexterity: 12,
        constitution: 12,
        intelligence: 10,
        wisdom: 10,
        charisma: 9,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common, Language::Goblin]),
        cr: 0.5,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        ..CreatureTemplate::defaults()
    }
});

/// Hobgoblin Captain — CR 3 martial humanoid, and the rank the
/// hobgoblin line was missing.
///
/// The bestiary already had a rank-and-file hobgoblin at CR ½ and a
/// Warlord at CR 6, which left a five-point hole exactly where a
/// warband's officer belongs. The captain fills it, and 5.2 gives it a
/// character rather than a stat bump: everything it carries is
/// **poisoned**. Nine slashing and a d6 of venom from the greatsword,
/// six piercing and 2d4 from the bow — the arrows carry more than the
/// blade does, which is the stat block telling you where it would
/// rather be standing.
///
/// Action lanes:
/// - **hobgoblin captain multiattack** — two greatsword swings.
/// - **captain greatsword** — the single swing, 2d6 + 1d6 poison.
/// - **captain longbow** — 1d8 + 2d4 poison at range.
///
/// **Aura of Authority** is cut. RAW: "while in a 10-foot Emanation
/// originating from the hobgoblin, the hobgoblin and its allies have
/// Advantage on attack rolls and saving throws." The engine's aura
/// cohorts hand out *numbers* — Aura of Protection's save bonus, Aura
/// of Hate's damage — and there is no lane that grants advantage to a
/// radius; a version that added a flat bonus instead would be a
/// different trait wearing this one's name. It is the clause to reach
/// for first if an advantage-granting aura ever lands.
///
/// Stat shape: AC 17 (half plate), 58 HP (9d8+18), STR 15 / DEX 14 /
/// CON 14 / INT 12 / WIS 10 / CHA 13. Speed 30. Darkvision 60. CR 3.
pub static HOBGOBLIN_CAPTAIN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*HOBGOBLIN_CAPTAIN_MULTI);
    actions.push(&HOBGOBLIN_CAPTAIN_GREATSWORD);
    actions.push(&HOBGOBLIN_CAPTAIN_LONGBOW);
    CreatureTemplate {
        name: "Hobgoblin Captain",
        // 'h' (lowercase) — 'H' is the rank-and-file hobgoblin's, and
        // the captain takes the other case of the same letter so the
        // two read as one warband on a board that holds both.
        glyph: 'h',
        ac: 17,
        // 9d8+18 = 58 average per SRD 5.2 (CR 3).
        hitpoints: "9d8+18".parse().unwrap(),
        speed: 30.,
        strength: 15,
        dexterity: 14,
        constitution: 14,
        intelligence: 12,
        wisdom: 10,
        charisma: 13,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common, Language::Goblin]),
        cr: 3.0,
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
    use crate::engine::types::{Coordinate, DamageType};

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
    fn hobgoblin_captain_template_shape() {
        let a = make(&HOBGOBLIN_CAPTAIN_TEMPLATE);
        assert_eq!(a.cr(), 3.0);
        assert!(a.find_action("hobgoblin captain multiattack").is_some());
        assert!(a.find_action("captain longbow").is_some());
    }

    /// Both of the captain's weapons carry poison, which is the whole
    /// of what 5.2 gives the rank over the rank and file.
    ///
    /// Asserted through `damage_types` rather than by rolling, because
    /// what is being pinned is the stat block's shape: a captain whose
    /// blade were plain steel would still hit for the same average and
    /// would be a different creature.
    #[test]
    fn everything_the_captain_carries_is_poisoned() {
        let a = make(&HOBGOBLIN_CAPTAIN_TEMPLATE);
        for name in ["captain greatsword", "captain longbow"] {
            let act = a.find_action(name).expect(name);
            assert!(
                act.damage_types().contains(&DamageType::Poison),
                "{} should carry venom",
                name
            );
        }
    }
}
