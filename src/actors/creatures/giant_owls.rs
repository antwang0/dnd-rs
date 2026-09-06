use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::GIANT_OWL_TALONS;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Giant Owl — CR ¼ large beast. The "moonlit harbinger" aerial scout
/// tier of large bird: a fast (fly 60) large frame with 2d6 talons
/// and 120-ft darkvision. Slots beside the Giant Eagle (CR 1 with
/// keen-sight aerial striker) and the Pteranodon (CR ¼ flying lizard)
/// on the flying-beast bench at the low-CR end — the canonical
/// nocturnal aerial scout that hunts in pitch-black canopy.
///
/// Action lane:
/// - **giant owl talons** — STR-based 2d6+STR slashing melee via the
///   shared `GIANT_OWL_TALONS` static. The CR-¼ owl's only swing.
///   Chunky single-hit damage on a fragile 19-HP large frame — the
///   owl trades survivability for swing weight, mirroring the Giant
///   Vulture / Giant Bat aerial-beast envelope at the same CR tier.
///
/// **Flyby** (RAW: doesn't provoke OAs when leaving an enemy's reach)
/// is on, via `FLYBY_TAG`, and it is what the "fast aerial harasser"
/// silhouette was always describing: the owl swoops in, takes its
/// talons swing, and leaves without paying the halberd. Free every
/// turn, where a Disengage costs an action.
///
/// **Keen Hearing and Sight** (advantage on Perception checks using
/// hearing or sight) stays flavor-only — skill checks don't route
/// through combat.
///
/// Defensive identity: AC 12 (light + agile), 19 HP (3d10+3). Vanilla
/// beast envelope — no resistances or condition immunities. The owl
/// is two-shot fragile; threat lives in mobility + the chunky talons,
/// not survivability.
///
/// **Darkvision 120** — outpaces the standard 60-ft envelope of most
/// nocturnal beasts (Hawk, Giant Bat) so the giant owl can spot prey
/// across the full pitch-dark forest at canopy distance. Routes
/// through the same `SpecialSense::Darkvision` chokepoint.
///
/// RAW the giant owl understands but doesn't speak Common, Elvish,
/// and Sylvan. We list those languages on the template — the engine
/// uses language sets for speech-tag gating (Suggestion, Command,
/// telepathy targeting) rather than as a "can be spoken to" flag,
/// so the listing accurately reflects "can be addressed in these
/// tongues."
///
/// Stat shape: AC 12, ~19 HP (3d10+3), STR 13, DEX 15, CON 12,
/// INT 10, WIS 14, CHA 10. Speed 5 (walking) collapsed to 30
/// (engine doesn't track separate fly speed; the canonical aerial
/// move-budget rounds up to the 30 floor). Senses: Darkvision 120.
/// Languages: Common, Elvish, Sylvan. Size Large. CR ¼. XP: 50 per
/// RAW.
pub static GIANT_OWL_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GIANT_OWL_TALONS);
    CreatureTemplate {
        name: "Giant Owl",
        // 'O' (uppercase) — shared with Ogre / Oni / Otyugh / Owlbear
        // cohort. The team color disambiguates on the map; the owl's
        // beast / fey context separates it from the giant / fiend
        // cohort at the prompt layer. Lowercase 'o' is taken by Orc.
        // Uppercase 'O' reads as "large rounded bird-of-prey" at the
        // small UI scale.
        glyph: 'O',
        ac: 12,
        // 3d10+3 = 19 average per MM (CR ¼).
        hitpoints: "3d10+3".parse().unwrap(),
        // RAW: walking 5 ft, fly 60 ft. The engine doesn't split
        // walking vs flying speeds; we collapse to fly 60 since the
        // owl spends almost every encounter aloft.
        // RAW speed line: Speed 5 ft., fly 60 ft.
        speed: 5.0,
        fly_speed: 60.0,
        strength: 13,
        intelligence: 10,
        dexterity: 15,
        wisdom: 14,
        constitution: 12,
        charisma: 10,
        // Darkvision 120 — twice the standard nocturnal-beast envelope.
        // Defines the "hunts in pitch-black forest canopy" niche.
        senses: HashSet::from([SpecialSense::Darkvision(120)]),
        languages: HashSet::from([Language::Common, Language::Elvish, Language::Sylvan]),
        cr: 0.25,
        size: Size::Large,
        // 5e Mounted Combat: MM's night-flying mount, on the same terms as the giant eagle.
        mountable: true,
        creature_type: CreatureType::Beast,
        actions,
        // 5e **Flyby**: "doesn't provoke an opportunity attack when it
        // flies out of an enemy's reach." Read by the mover-side
        // suppression lane in `dispatch_opportunity_attacks`, and gated
        // there on the creature actually being airborne.
        features: HashSet::from([crate::actions::class_features::FLYBY_TAG]),
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
            &GIANT_OWL_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn giant_owl_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.25);
        assert_eq!(a.size(), Size::Large);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("giant owl talons").is_some());
    }

    #[test]
    fn giant_owl_has_long_darkvision() {
        // Pin the load-bearing sensory trait: Darkvision 120 anchors
        // the owl's "pitch-black canopy hunter" identity. The standard
        // 60-ft envelope of most nocturnal beasts (Hawk, Giant Bat)
        // wouldn't let the owl spot prey across a forest clearing.
        // A future template refactor that quietly trimmed the
        // darkvision to 60 would erase the night-hunter niche.
        let a = make();
        assert!(a.senses().contains(&SpecialSense::Darkvision(120)));
    }
}
