use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    CONSTRICTOR_SNAKE_BITE, CONSTRICTOR_SNAKE_CONSTRICT, GIANT_CONSTRICTOR_SNAKE_BITE,
    GIANT_CONSTRICTOR_SNAKE_CONSTRICT,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Constrictor Snake — CR ¼ large beast. The jungle's silent ambush
/// predator: a 12-foot scaled coil that strikes from the canopy, then
/// wraps prey in crushing folds. Slots alongside the other low-CR beast
/// fillers (Boar / Wolf / Stirge / Giant Crab / Hyena) at the bottom of
/// the encounter pool. First snake-shaped creature in the pool — the
/// only adjacent entry is the Cockatrice (Tiny monstrosity, not a true
/// snake).
///
/// Action lanes:
/// - **constrictor snake bite** — STR-based 1d6+STR piercing melee.
///   Vanilla `SimpleWeapon` — the bite is pure damage; the grapple lane
///   lives on the separate `constrict` action.
/// - **constrict** — STR-based 1d8+STR bludgeoning melee with a DC 14
///   STR save-or-Grappled rider (10 rounds). The save + condition
///   install routes through the shared `save_or_condition_rider`
///   chokepoint so grapple-immune targets (the elemental / construct
///   envelope) shrug it off cleanly.
///
/// Defensive identity: AC 12, ~13 HP (2d10+2). Standard beast envelope
/// — no special resistances, no condition immunities. The constrictor's
/// threat is the grapple lock-down, not damage soak.
///
/// Stat shape: AC 12, ~13 HP (2d10+2), STR 15, DEX 14, CON 12, INT 1,
/// WIS 10, CHA 3. Speed 30 (RAW also swim 30 which we don't model as a
/// separate movement lane). Senses: Blindsight 10ft (the snake's heat-
/// pit sensors). Size Large. CR ¼.
pub static CONSTRICTOR_SNAKE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&CONSTRICTOR_SNAKE_BITE);
    actions.push(&*CONSTRICTOR_SNAKE_CONSTRICT);
    CreatureTemplate {
        name: "Constrictor Snake",
        // 'n' (lowercase) — distinct from 'N' (Nalfeshnee / Nothic) and
        // 's' (Steam Mephit / Skeleton). 'n' reads as the low coiled
        // silhouette of a serpent.
        glyph: 'n',
        ac: 12,
        // 2d10+2 ≈ 13 average per MM (CR ¼).
        hitpoints: "2d10+2".parse().unwrap(),
        speed: 30.,
        strength: 15,
        intelligence: 1,
        dexterity: 14,
        wisdom: 10,
        constitution: 12,
        charisma: 3,
        senses: HashSet::from([SpecialSense::Blindsight(10)]),
        languages: HashSet::new(),
        cr: 0.25,
        size: Size::Large,
        creature_type: CreatureType::Beast,
        actions,
        ..CreatureTemplate::defaults()
    }
});

/// Giant Constrictor Snake — CR 2 huge beast. The titanic apex predator
/// of jungle / swamp / rainforest: a 30-foot scaled python that can
/// swallow a humanoid whole. Slots between the regular Constrictor (CR
/// ¼) and the Giant Toad / Polar Bear (CR 2) on the upper-low beast
/// bench, and just below the Giant Ape (CR 7) and Roc (CR 11) on the
/// "huge beast" ladder.
///
/// Action lanes:
/// - **giant constrictor snake bite** — STR-based 2d6+STR piercing at
///   reach 2 tiles (the huge serpent's lunge), with a flat 1d4 poison
///   rider (RAW 2d4 poison collapsed for typed-resistance clarity).
/// - **giant constrict** — STR-based 2d8+STR bludgeoning at reach 2
///   tiles with a DC 16 STR save-or-Grappled rider (10 rounds). Higher
///   DC than the regular constrictor's DC 14 — the giant snake's coils
///   are much harder to break out of.
///
/// Defensive identity: AC 12, ~60 HP (8d12+8). Standard beast envelope
/// — no special resistances, no condition immunities. The giant snake's
/// threat is the reach-2 grapple lock-down on a wide HP bar.
///
/// Stat shape: AC 12, ~60 HP (8d12+8), STR 19, DEX 14, CON 12, INT 1,
/// WIS 10, CHA 3. Speed 30 (RAW also swim 30 which we don't model).
/// Senses: Blindsight 10ft. Size Huge. CR 2.
pub static GIANT_CONSTRICTOR_SNAKE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*GIANT_CONSTRICTOR_SNAKE_BITE);
    actions.push(&*GIANT_CONSTRICTOR_SNAKE_CONSTRICT);
    CreatureTemplate {
        name: "Giant Constrictor Snake",
        // 'N' (uppercase) — sibling glyph to the 'n' constrictor; the
        // huge variant gets the capital for its larger silhouette,
        // matching the wolf/dire-wolf naming style.
        glyph: 'N',
        ac: 12,
        // 8d12+8 ≈ 60 average per MM (CR 2).
        hitpoints: "8d12+8".parse().unwrap(),
        speed: 30.,
        strength: 19,
        intelligence: 1,
        dexterity: 14,
        wisdom: 10,
        constitution: 12,
        charisma: 3,
        senses: HashSet::from([SpecialSense::Blindsight(10)]),
        languages: HashSet::new(),
        cr: 2.0,
        size: Size::Huge,
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
    use crate::engine::types::{Coordinate, DamageType};

    #[test]
    fn constrictor_snake_template_shape() {
        let a = ActorInstance::from_creature_template(
            &CONSTRICTOR_SNAKE_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 0.25);
        assert_eq!(a.size(), Size::Large);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("constrictor snake bite").is_some());
        assert!(a.find_action("constrict").is_some());
    }

    #[test]
    fn giant_constrictor_snake_template_shape() {
        let a = ActorInstance::from_creature_template(
            &GIANT_CONSTRICTOR_SNAKE_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 2.0);
        assert_eq!(a.size(), Size::Huge);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("giant constrictor snake bite").is_some());
        assert!(a.find_action("giant constrict").is_some());
        // The bite is poisoned — the giant variant carries the venom
        // rider, the regular constrictor's bite is pure piercing.
        let bite = a.find_action("giant constrictor snake bite").unwrap();
        assert!(bite.damage_types().contains(&DamageType::Poison));
    }
}
