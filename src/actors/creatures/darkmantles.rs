use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::DARKMANTLE_CRUSH;
use crate::actions::spells::DARKNESS;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Darkmantle — CR ½ small monstrosity. A cave ceiling that turns out to
/// be alive, drops onto a head, and smothers it. The cheapest creature
/// in the bestiary that fights by taking away sight.
///
/// Action lanes:
/// - **darkness aura** — RAW's innate Darkness, once per day, centred
///   on itself. The engine carries Darkness as an ordinary level-2 zone
///   spell, so the darkmantle gets a single level-2 slot and casts the
///   real thing: a sphere of magical dark that no nonmagical light lifts
///   and no darkvision penetrates.
/// - **darkmantle crush** — 1d6+STR bludgeoning that blinds on hit.
///
/// The two lanes are the same tactic twice, and that redundancy is the
/// creature. A darkmantle that has cast its Darkness is fighting inside
/// a sphere where its **Blindsight 60** is the only working sense on the
/// board — every combatant in the cloud is effectively Blinded and it is
/// not — and a darkmantle that has spent its slot still blinds whatever
/// it lands on.
///
/// **Echolocation** (RAW: the darkmantle's blindsight fails while
/// deafened) is the clause not carried; the engine's Blindsight has no
/// deafness gate, and installing one for a single CR ½ creature would be
/// a rule only the darkmantle obeys.
///
/// Stat shape per the SRD: AC 11, 22 HP (5d8), STR 16 / DEX 12 / CON 13
/// / INT 2 / WIS 10 / CHA 5. Speed 10, fly 30 — the fly speed is what
/// gets it onto the ceiling, and the engine's single speed field takes
/// the larger of the two. Blindsight 60 (blind beyond). CR ½.
pub static DARKMANTLE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*DARKNESS);
    actions.push(&DARKMANTLE_CRUSH);
    CreatureTemplate {
        name: "Darkmantle",
        // 'k' (lowercase) — an unclaimed letter. 'd' and 'D' are the
        // dragon / drake / dire-wolf pools and 'm' is the caster band.
        glyph: 'k',
        ac: 11,
        // 5d8 ≈ 22 average per the SRD (CR ½).
        hitpoints: "5d8".parse().unwrap(),
        // RAW walks at 10 and flies at 30. The engine has one speed, and
        // the flying one is the one a darkmantle spends its turn using.
        speed: 30.,
        strength: 16,
        dexterity: 12,
        constitution: 13,
        intelligence: 2,
        wisdom: 10,
        charisma: 5,
        senses: HashSet::from([SpecialSense::Blindsight(60)]),
        cr: 0.5,
        size: Size::Small,
        creature_type: CreatureType::Monstrosity,
        actions,
        // One level-2 slot: RAW's "1/day" Darkness, priced in the only
        // currency the engine has for an innate cast.
        spell_slots_by_level: vec![0, 1],
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
            &DARKMANTLE_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn darkmantle_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.5);
        assert_eq!(a.size(), Size::Small);
        assert_eq!(a.creature_type(), CreatureType::Monstrosity);
        assert!(a.find_action("darkmantle crush").is_some());
    }

    /// The darkmantle's Darkness is only a weapon because it can see
    /// through what it casts. Both halves asserted together: a
    /// darkmantle with the slot and no blindsight would be blinding
    /// itself, which is the opposite of the creature.
    #[test]
    fn the_darkmantle_can_see_inside_the_dark_it_makes() {
        let a = make();
        assert!(a.find_action("darkness").is_some());
        assert_eq!(a.spell_slot_manager.spell_slots(2).spell_slots, 1);
        assert!(
            a.senses().iter().any(|s| matches!(s, SpecialSense::Blindsight(r) if *r >= 60)),
            "the darkmantle needs blindsight to fight in its own cloud"
        );
    }
}
