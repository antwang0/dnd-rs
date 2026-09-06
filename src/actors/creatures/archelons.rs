use crate::actions::class_features::{SWIM_SPEED_TAG, UNDERWATER_BREATHING_TAG};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{ARCHELON_BITE, ARCHELON_MULTI};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, Skill};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Archelon — CR 4 huge beast. A prehistoric sea turtle with AC 17, the
/// highest natural armour class on the whole beast shelf, and Stealth
/// proficiency on a Dexterity 16 frame, which is not a combination
/// anything else at this tier has.
///
/// The result is an ambusher that is genuinely hard to hit. Two 3d6+4
/// bites a turn is ordinary damage for CR 4 — the hippopotamus does
/// half again as much — and the archelon expects to be taking swings
/// for several more rounds than the hippo does.
///
/// Action lanes:
/// - **archelon multiattack** — two bites per Action.
/// - **archelon bite** — STR-based 3d6+STR piercing.
///
/// **Amphibious** (RAW: breathes air and water) is flavor here, as it
/// is on every amphibious template — water is terrain in this engine
/// rather than an atmosphere. The swim speed is the half with content:
/// free movement through `TerrainType::Water` and no underwater melee
/// penalty.
///
/// Speed is RAW's land speed of 20 rather than its swim speed of 80,
/// following the convention the crocodile set — a creature that
/// genuinely walks keeps its walking number, and the swim tag carries
/// the rest. On a water map that makes the archelon slow to reach and
/// unpleasant to fight once reached, which is about right for an
/// ambush predator that lives in the sea.
///
/// Stat shape: AC 17, ~90 HP (12d12+12), STR 18, DEX 16, CON 13,
/// INT 4, WIS 14, CHA 6. Speed 20 (swim 80 in RAW). Skills Stealth.
/// Size Huge. CR 4. XP 1,100 per RAW.
pub static ARCHELON_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&ARCHELON_BITE);
    actions.push(&*ARCHELON_MULTI);
    CreatureTemplate {
        name: "Archelon",
        // 'C' — the domed shell in uppercase, above the crab's
        // lowercase 'c' on the shelled cohort.
        glyph: 'C',
        ac: 17,
        hitpoints: "12d12+12".parse().unwrap(),
        speed: 20.,
        strength: 18,
        intelligence: 4,
        dexterity: 16,
        wisdom: 14,
        constitution: 13,
        charisma: 6,
        skills: HashSet::from([Skill::Stealth]),
        cr: 4.0,
        size: Size::Huge,
        creature_type: CreatureType::Beast,
        actions,
        features: HashSet::from([SWIM_SPEED_TAG, UNDERWATER_BREATHING_TAG]),
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
            &ARCHELON_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn archelon_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 4.0);
        assert_eq!(a.size(), Size::Huge);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("archelon multiattack").is_some());
        assert!(a.has_swim_speed());
    }

    /// AC 17 is the archelon's whole defensive identity and the highest
    /// on the beast shelf — the shell is the stat block.
    #[test]
    fn the_shell_is_the_stat_block() {
        assert_eq!(make().armor_class(), 17);
    }
}
