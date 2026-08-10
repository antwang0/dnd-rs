use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::RIDING_HORSE_HOOVES;
use crate::actors::actor_template::CreatureTemplate;
use crate::actors::creatures::riding_horses::RIDING_HORSE_TEMPLATE;
use std::sync::LazyLock;

/// The Phantom Steed's walking speed, in feet.
///
/// RAW, and the entire reason the spell exists: "The creature has the
/// statistics of a riding horse, but it has a speed of 100 feet."
/// Named rather than inlined because it is the one number that
/// separates this template from the chassis it clones, and the test
/// below reads it rather than repeating the literal.
///
/// A hundred feet is the fastest walking speed in the engine — half
/// again the warhorse's sixty, and faster than every flier on the
/// bestiary bar the couatl. On the 2.5 ft grid that is forty tiles of
/// movement in a turn, which is most of the width of a generated map.
pub const PHANTOM_STEED_SPEED: f32 = 100.;

/// Phantom Steed — the quasi-real mount conjured by the level-3
/// illusion of the same name (`spells::PHANTOM_STEED`).
///
/// RAW is a one-sentence stat block: "A Large quasi-real, horselike
/// creature appears on the ground in an unoccupied space of your
/// choice within range. […] The creature has the statistics of a
/// riding horse, but it has a speed of 100 feet." So this template is
/// `RIDING_HORSE_TEMPLATE` with two fields moved — the name and the
/// speed — and the rest inherited through the clone tail. Writing the
/// thirteen unchanged stats out again would be a second copy of the
/// riding horse that a future correction to the horse would silently
/// miss.
///
/// **It is still a Beast**, which looks wrong for something conjured
/// out of nothing and is the deliberate reading of "has the statistics
/// of a riding horse". Creature type is a statistic — it decides what
/// Hold Person refuses to touch, what the ranger's Slayer's Prey can
/// name, and what a druid's beast-only lists can reach — and RAW hands
/// the caster the horse's statistics without carving out an exception
/// for that one. The alternative readings (Monstrosity for
/// "quasi-real", Construct for "conjured") would each make the steed
/// respond differently to a dozen spells than the horse it is supposed
/// to be a copy of, on the strength of an adjective in the flavour
/// text.
///
/// What separates it from an ordinary mount is not the stat block but
/// the price: three tiles of movement per action-point on a chassis
/// nobody had to lead into the dungeon, no concentration held, and it
/// vanishes with the spell rather than bleeding out. See
/// `spells::PHANTOM_STEED` for the spell-side shape and for the two
/// RAW clauses (the ritual cast, the one-minute fade) that have no
/// combat surface.
///
/// Glyph 'H', shared with the riding horse and the warhorse it is a
/// copy of. The equine cohort reads as one thing on the map on
/// purpose: a player who sees an 'H' beside a caster should think
/// "somebody is about to get on that", and which horse it is matters
/// less than that it is a horse.
pub static PHANTOM_STEED_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Rebuilt rather than cloned off the chassis's `actions`, because
    // `DEFAULT_ACTIONS.clone()` plus the one attack is what the riding
    // horse itself does and cloning a clone buys nothing.
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&RIDING_HORSE_HOOVES);
    CreatureTemplate {
        name: "Phantom Steed",
        speed: PHANTOM_STEED_SPEED,
        actions,
        ..RIDING_HORSE_TEMPLATE.clone()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::{Coordinate, CreatureType, Size};

    fn make(template: &'static LazyLock<CreatureTemplate>) -> ActorInstance {
        ActorInstance::from_creature_template(
            template,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn the_phantom_steed_is_a_riding_horse_that_runs() {
        let steed = make(&PHANTOM_STEED_TEMPLATE);
        assert_eq!(steed.speed(), PHANTOM_STEED_SPEED);
        assert!(
            steed.speed() > make(&RIDING_HORSE_TEMPLATE).speed(),
            "the speed is the whole spell"
        );
        assert!(steed.find_action("horse hooves").is_some());
    }

    /// The clone tail is load-bearing: everything the spell does *not*
    /// name has to still be the riding horse's. A future edit that
    /// spelled the stats out by hand would drift, and the first thing
    /// to break would be the half of the template nobody reads.
    #[test]
    fn everything_the_spell_does_not_name_is_still_the_horse() {
        let steed = make(&PHANTOM_STEED_TEMPLATE);
        let horse = make(&RIDING_HORSE_TEMPLATE);
        assert_eq!(steed.armor_class(), horse.armor_class());
        assert_eq!(steed.cr(), horse.cr());
        assert_eq!(steed.size(), Size::Large);
        assert_eq!(steed.creature_type(), CreatureType::Beast);
        assert!(
            PHANTOM_STEED_TEMPLATE.mountable,
            "a steed nobody can ride is not a steed"
        );
    }
}
