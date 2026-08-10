use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::GIANT_FIRE_BEETLE_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Giant Fire Beetle — CR 0 small beast. Four hit points and a 1d6
/// bite, and a pair of glands above its eyes that shed light whether
/// the beetle wants them to or not.
///
/// The **Illumination** is the reason to have one. RAW: "the beetle
/// sheds bright light in a 10-foot radius and dim light for an
/// additional 10 feet", and it is the only creature in the game whose
/// corpse is famously a lamp — the light persists for a day after it
/// dies, which is why dungeon-dwellers harvest them. The engine does
/// not model the corpse half (the glow rides `innate_light` and goes
/// out with the beetle, like every other body that is its own light
/// source), but the live half is the interesting one: a CR 0 beast
/// that cannot be snuck up on, cannot sneak, and lights up whatever it
/// is standing next to.
///
/// Stat shape per the SRD: AC 13 (natural armor), 4 HP (1d6+1), STR 8 /
/// DEX 10 / CON 12 / INT 1 / WIS 7 / CHA 3. Speed 30. Blindsight 30.
/// CR 0.
pub static GIANT_FIRE_BEETLE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GIANT_FIRE_BEETLE_BITE);
    CreatureTemplate {
        name: "Giant Fire Beetle",
        // 'b' (lowercase) — free; 'B' is the devil band and 'z' the
        // Azer's.
        glyph: 'b',
        ac: 13,
        // 1d6+1 ≈ 4 average per the SRD (CR 0).
        hitpoints: "1d6+1".parse().unwrap(),
        speed: 30.,
        strength: 8,
        dexterity: 10,
        constitution: 12,
        intelligence: 1,
        wisdom: 7,
        charisma: 3,
        senses: HashSet::from([SpecialSense::Blindsight(30)]),
        cr: 0.0,
        size: Size::Small,
        creature_type: CreatureType::Beast,
        actions,
        // Illumination — 10 ft bright, 10 ft dim. The same radii the
        // azer and the magmin carry, on the only beast that has them.
        innate_light: Some((
            crate::engine::lighting::GLOW_BRIGHT_TILES,
            crate::engine::lighting::GLOW_DIM_TILES,
        )),
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
    fn giant_fire_beetle_template_shape() {
        let a = ActorInstance::from_creature_template(
            &GIANT_FIRE_BEETLE_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 0.0);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("giant fire beetle bite").is_some());
    }

    /// The whole reason the stat block exists. A fire beetle without
    /// its glow is a worse rat.
    #[test]
    fn the_beetle_is_a_lamp() {
        assert_eq!(
            GIANT_FIRE_BEETLE_TEMPLATE.innate_light,
            Some((
                crate::engine::lighting::GLOW_BRIGHT_TILES,
                crate::engine::lighting::GLOW_DIM_TILES
            ))
        );
    }
}
