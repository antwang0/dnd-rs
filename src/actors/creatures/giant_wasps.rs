use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::GIANT_WASP_STING;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size};
use std::sync::LazyLock;

/// Giant Wasp — CR ½ medium beast. The "flying venom drone" tier of
/// insectoid: a fast aerial harasser with a single sting that lays
/// down a heavy save-or-3d6-poison rider AND installs the Poisoned
/// condition on a failed save. Slots beside the Giant Wolf Spider
/// (CR ¼ medium, 2d6 poison) and the Giant Centipede (CR ¼ small,
/// 3d6 poison) on the venom-rider arthropod bench — same chassis
/// shape, but on a Medium fly-50 frame instead of a slower
/// crawler, raising the CR to ½ and giving the wasp a credible
/// "swoop in, sting, retreat" combat lane.
///
/// Action lane:
/// - **giant wasp sting** — DEX-based 1d6+DEX piercing melee with a
///   DC 11 CON save-or-3d6-poison-AND-Poisoned rider via the shared
///   `WeaponWithSaveDamage::melee_with_condition` chassis. RAW pairs
///   this stinger's venom with a "Poisoned for 1 hour" rider gated on
///   the venom dropping the target to 0 HP — we promote to an
///   unconditional 10-round Poisoned install on a failed save (the
///   "Poisoned attack/ability rolls at disadvantage" envelope is the
///   load-bearing tactical clause; the 0-HP gate is a scope cut).
///   Same chassis-share lane as the Spider Bite / Ettercap Bite /
///   Drow Poisoned Crossbow.
///
/// Defensive identity: AC 13 (medium + DEX), 22 HP (5d8). Vanilla
/// beast envelope — no resistances or condition immunities. The
/// wasp dies to two solid hits; threat lives in the venom rider on
/// a fragile flying frame, not survivability. Pair with a melee
/// predator (Giant Centipede / Giant Wolf Spider) so the wasp
/// pressures squishy back-line targets from above while the
/// crawlers close in on the front.
///
/// Stat shape: AC 13, ~22 HP (5d8), STR 10, DEX 22, CON 10, INT 1,
/// WIS 10, CHA 3. Speed 50 — RAW: walking 10 ft + fly 50 ft. The
/// engine collapses ground + fly to a single per-creature speed;
/// we pin to the fly speed since wasps almost never walk in
/// encounter scope. Size Medium. CR ½. XP: 100 per RAW.
pub static GIANT_WASP_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GIANT_WASP_STING);
    CreatureTemplate {
        name: "Giant Wasp",
        // 'w' (lowercase) — small / medium flying-insect silhouette.
        // 'W' (uppercase) is taken by Wraith / Wight / Worg / Werewolf
        // cohort; lowercase 'w' reads as "fast buzzy nuisance" beside
        // 'b' (Bat), 'k' (Hawk) at the small UI scale. The team color
        // disambiguates from the same-glyph (Wolves use 'w' too) on
        // the map; the beast / CR-½ context further separates.
        glyph: 'w',
        ac: 13,
        // 5d8 = 22 average per MM (CR ½).
        hitpoints: "5d8".parse().unwrap(),
        // Fly 50 — slower than the Hawk / Giant Bat / Giant Owl
        // (60) but still firmly in the "aerial harasser" envelope.
        // Matches RAW's giant-wasp stat block. The engine collapses
        // ground + fly to a single per-creature speed.
        // RAW speed line: Speed 10 ft., fly 50 ft.
        speed: 10.0,
        fly_speed: 50.0,
        strength: 10,
        intelligence: 1,
        dexterity: 14,
        wisdom: 10,
        constitution: 10,
        charisma: 3,
        cr: 0.5,
        size: Size::Medium,
        creature_type: CreatureType::Beast,
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
            &GIANT_WASP_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn giant_wasp_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.5);
        assert_eq!(a.size(), Size::Medium);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("giant wasp sting").is_some());
    }

    #[test]
    fn giant_wasp_is_fast_flier() {
        // Pin the load-bearing mobility trait: fly 50 is the wasp's
        // identity. A future template-refactor that quietly dropped
        // the speed back to the default 30 would erase the "aerial
        // swoop-and-sting" silhouette (the wasp would become a
        // grounded medium-beast venom rider, indistinguishable from
        // the Giant Centipede / Giant Wolf Spider crawler bench at
        // a higher CR for no good reason).
        let a = make();
        assert!(a.speed() >= 50.0);
    }
}
