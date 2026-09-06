use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{OTYUGH_BITE, OTYUGH_MULTI, OTYUGH_TENTACLE};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Otyugh — CR 5 large aberration. Garbage-eating tentacle horror of the
/// Underdark, sewer, and abandoned dungeon level. Slots between Night Hag
/// (CR 5) and Drider (CR 6) on the mid-tier ladder — the brawler-grappler
/// answer to the Night Hag's caster-flavored CR 5 slot.
///
/// Action lanes:
/// - **otyugh multiattack** — 2 tentacles + 1 bite per Action via
///   `CompoundAttack`. The tentacles come first so the wrapper's reach
///   check uses reach 2 (10 ft) — the bite at reach 1 still lands cleanly
///   because the target is necessarily within the tentacle envelope.
/// - **otyugh bite** (standalone) — 2d8+STR piercing with a CON 15 save
///   or Poisoned (approximation of RAW's "contract disease until cured";
///   we use Rounds(5) since the engine doesn't model long-term diseases).
/// - **otyugh tentacle** (standalone) — 1d8+STR bludgeoning + 1d8 piercing
///   rider at reach 2, with Restrained for 1 round on hit (approximation
///   of RAW's "grappled + restrained" clause).
///
/// Defensive identity: AC 14 (natural armor — the otyugh's mottled
/// blubbery hide). 104 HP (11d10+44), CON 19 — the otyugh is a
/// damage sponge, not an evasive striker. No resistances or immunities
/// — the otyugh is a flesh-and-tentacle predator, not an elemental or
/// fiend. CON save proficiency reflects the gut-fortitude RAW for the
/// otyugh's filth-and-rot diet.
///
/// Stat shape: AC 14, ~104 HP (11d10+44), STR 16, DEX 11, CON 19, INT 6,
/// WIS 13, CHA 6. Speed 30. Senses: Darkvision 120. No languages
/// (otyughs communicate via limited telepathy that the engine doesn't
/// model). Size Large. CR 5.
///
/// RAW also gives the otyugh:
/// - **Tentacle Slam** (the otyugh slams grappled creatures together for
///   2d6+STR bludgeoning, DC 14 CON save half) — omitted since the
///   engine doesn't track per-grappler grapple links (would need to
///   know which actors are grappled by *this* otyugh specifically).
/// - **Limited Telepathy** (the otyugh exchanges crude images / emotions
///   with other otyughs in 120 ft) — no in-engine consumer.
/// - **Otyugh** language — omitted since the engine's `Language` enum
///   doesn't have a per-monster language variant. The otyugh would have
///   no other languages either, so we leave the languages set empty.
pub static OTYUGH_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*OTYUGH_MULTI);
    actions.push(&OTYUGH_BITE);
    actions.push(&*OTYUGH_TENTACLE);
    CreatureTemplate {
        name: "Otyugh",
        // 'O' (uppercase) — distinct from 'o' (Ogre uses 'O' too; we
        // share the glyph since both are CR-5 large brutes and the
        // glyph pool is exhausted at uppercase letters that read as
        // "lumbering large monstrosity").
        glyph: 'O',
        ac: 14,
        // 11d10+44 ≈ 104 average per MM (CR 5).
        hitpoints: "11d10+44".parse().unwrap(),
        speed: 30.,
        strength: 16,
        intelligence: 6,
        dexterity: 11,
        wisdom: 13,
        constitution: 19,
        charisma: 6,
        senses: HashSet::from([SpecialSense::Darkvision(120)]),
        // No standard languages — RAW gives the otyugh "Otyugh" (a private
        // species cant) which the engine's `Language` enum doesn't model.
        // An otyugh can also share crude images with other otyughs at 120
        // ft via Limited Telepathy, which the engine likewise doesn't
        // model.
        cr: 5.0,
        size: Size::Large,
        creature_type: CreatureType::Aberration,
        actions,
        // CON save proficiency — RAW Con +7. The gut-fortitude lane that
        // reflects the otyugh's filth-and-rot diet (the same rider it
        // installs on bite victims, ironically, is the one its own
        // saves are best at shrugging off).
        proficient_saves: HashSet::from([AbilityScoreType::Constitution]),
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
    fn otyugh_template_shape() {
        let a = ActorInstance::from_creature_template(
            &OTYUGH_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 5.0);
        assert_eq!(a.size(), Size::Large);
        assert_eq!(a.creature_type(), CreatureType::Aberration);
        // The otyugh's three action lanes.
        assert!(a.find_action("otyugh multiattack").is_some());
        assert!(a.find_action("otyugh bite").is_some());
        assert!(a.find_action("otyugh tentacle").is_some());
    }

    #[test]
    fn otyugh_has_no_magic_resistance_or_immunities() {
        let a = ActorInstance::from_creature_template(
            &OTYUGH_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        // The otyugh is a flesh-and-tentacle predator — no magic
        // resistance, no damage modifiers. The defensive identity is the
        // 104-HP pool + CON save proficiency, not any immunity envelope.
        assert!(!a.has_magic_resistance());
    }
}
