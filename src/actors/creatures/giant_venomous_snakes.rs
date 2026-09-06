use crate::actions::class_features::SWIM_SPEED_TAG;
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::GIANT_VENOMOUS_SNAKE_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Giant Venomous Snake — CR ¼ medium beast. The "ambush viper" tier
/// of serpent: a fast medium frame with a venomous bite and
/// Blindsight 10 + Darkvision 60 senses. Slots beside the Giant
/// Centipede (CR ¼ small venom-glass-cannon) and the Giant Wolf
/// Spider (CR ¼ medium venom-rider) on the venom-flavored low-CR
/// ambusher bench — the canonical CR-¼ coiled-strike serpent. Sister
/// to the Constrictor Snake (CR ¼ — non-venomous grappler) in the
/// snake family.
///
/// Action lane:
/// - **giant venomous snake bite** — DEX-based 1d4+DEX piercing
///   melee with a DC 11 CON save-or-3d6-poison rider via the shared
///   `WeaponWithSaveDamage` chassis. Same "one save gates both base
///   and rider damage" shape as the Giant Wolf Spider / Spider Bite /
///   Giant Centipede cohort; only the dice / DC / reach differ. The
///   3d6 poison dice are load-bearing — a failed save can drop a
///   wounded low-level target in one swing. Reach 2 (10 ft) lets the
///   snake strike from a coil one tile away — distinct from the
///   centipede / spider's 1-tile melee, matching RAW's serpent
///   reach.
///
/// Defensive identity: AC 14 (small + agile + scaled), 11 HP
/// (2d8+2). Vanilla beast envelope — no resistances or condition
/// immunities. The snake dies to a single solid hit; threat lives
/// in the venom rider, not survivability. **Blindsight 10** lets
/// the snake sense vibration through pit-organ-equivalent
/// echolocation at close range; **Darkvision 60** covers night /
/// dungeon biomes.
///
/// Stat shape: AC 14, ~11 HP (2d8+2), STR 10, DEX 18, CON 13,
/// INT 2, WIS 10, CHA 3. Speed 30 walking + swim 30 (magnitudes
/// collapsed to walking 30 since the engine doesn't track a swimming
/// separately). Senses: Blindsight 10, Darkvision 60. Size Medium.
/// CR ¼. XP: 50 per RAW.
pub static GIANT_VENOMOUS_SNAKE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GIANT_VENOMOUS_SNAKE_BITE);
    CreatureTemplate {
        name: "Giant Venomous Snake",
        // 's' (lowercase) — small serpentine silhouette. Shared with
        // Sprite ('s'); the team color disambiguates on the map and
        // the beast / fey CR contexts rarely collide. Uppercase 'S'
        // is taken by Salamander / Spectator / Stirge cohort.
        glyph: 's',
        ac: 14,
        // 2d8+2 = 11 average per MM (CR ¼).
        hitpoints: "2d8+2".parse().unwrap(),
        speed: 30.,
        strength: 10,
        intelligence: 2,
        dexterity: 18,
        wisdom: 10,
        constitution: 13,
        charisma: 3,
        // Blindsight 10 — pit-organ vibration sense at close range.
        // Darkvision 60 — night / dungeon biome coverage.
        senses: HashSet::from([
            SpecialSense::Blindsight(10),
            SpecialSense::Darkvision(60),
        ]),
        cr: 0.25,
        size: Size::Medium,
        creature_type: CreatureType::Beast,
        actions,
        // RAW swim speed: the tag is what makes `TerrainType::Water`
        // free to cross and lifts the underwater melee penalty.
        features: HashSet::from([SWIM_SPEED_TAG]),
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
            &GIANT_VENOMOUS_SNAKE_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn giant_venomous_snake_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.25);
        assert_eq!(a.size(), Size::Medium);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("giant venomous snake bite").is_some());
    }

    #[test]
    fn giant_venomous_snake_has_blindsight() {
        // Pin the load-bearing sensory trait: Blindsight 10 lets the
        // snake ambush in pitch dark even at point-blank — vital for
        // the "coiled in a dungeon corner" reveal. A future template
        // refactor that quietly stripped Blindsight would demote the
        // snake to a slightly-better Constrictor — flattening the
        // tactical contrast between the two snake-family entries.
        let a = make();
        assert!(a.senses().contains(&SpecialSense::Blindsight(10)));
    }
}
