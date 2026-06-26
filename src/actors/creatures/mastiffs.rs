use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::MASTIFF_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size};
use std::sync::LazyLock;

/// Mastiff — CR ⅛ medium beast. The "guard dog" tier of canine: a
/// trained hunting / herding hound. Heavier-jawed than the vanilla
/// Wolf (1d6 die vs 1d4) but lacks Pack Tactics — the mastiff is a
/// trained companion, not a pack-hunting wild canid. Slots between
/// the Wolf (CR ¼ with Pack Tactics) and the Hyena (CR 0 pack
/// scavenger) on the small-canine bench — the canonical "loyal
/// hound" beast for shepherd / watchman / hunter encounters.
///
/// Action lane:
/// - **mastiff bite** — STR-based 1d6+STR piercing melee via the
///   shared `MASTIFF_BITE` static (`WeaponWithSaveCondition` chassis,
///   DC 11 STR save-or-Prone trip rider). Same trip shape as Wolf /
///   Dire Wolf / Worg — the canonical "bite knocks the target down"
///   loop centralized at the shared chassis. The 1d6 die makes the
///   mastiff's single hit a credible mid-fight threat (heavier than
///   the wolf's 1d4) without the Pack-Tactics multiplier.
///
/// **Keen Hearing and Smell** — RAW: "The mastiff has advantage on
/// Wisdom (Perception) checks that rely on hearing or smell." The
/// engine doesn't surface skill checks through combat, so the trait
/// stays flavor-only — the load-bearing combat clause (the
/// save-or-Prone bite) defines the mastiff's per-round footprint.
///
/// Defensive identity: AC 12 (small + DEX-driven), 5 HP (1d8+1).
/// Vanilla beast envelope — no resistances or condition immunities.
/// The mastiff dies to a single solid hit; its threat lives in the
/// trip-bite combo that strands a target Prone for the rest of the
/// party to capitalize on.
///
/// Stat shape: AC 12, ~5 HP (1d8+1), STR 13, DEX 14, CON 12, INT 3,
/// WIS 12, CHA 7. Speed 40 (slightly faster than the wolf's 30 —
/// the mastiff is bred for endurance pursuit, not a sprint pack).
/// Size Medium. CR ⅛. XP: 25 per RAW.
pub static MASTIFF_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&MASTIFF_BITE);
    CreatureTemplate {
        name: "Mastiff",
        // 'M' — distinct from 'm' (Magmin / Mephit cohort) and from 'W'
        // (Wolf / Wereperson). Capital 'M' reads as "medium-sized
        // canine silhouette" at the small UI scale, mirroring 'B'
        // (Bandit / Bear), 'D' (Dire Wolf) etc.
        glyph: 'M',
        ac: 12,
        // 1d8+1 ≈ 5 average per MM (CR ⅛).
        hitpoints: "1d8+1".parse().unwrap(),
        speed: 40.,
        strength: 13,
        intelligence: 3,
        dexterity: 14,
        wisdom: 12,
        constitution: 12,
        charisma: 7,
        cr: 0.125,
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
            &MASTIFF_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn mastiff_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.125);
        assert_eq!(a.size(), Size::Medium);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("mastiff bite").is_some());
    }

    #[test]
    fn mastiff_lacks_pack_tactics() {
        // Pin the deliberate omission: the mastiff is a trained
        // companion, not a pack hunter. The Wolf at CR ¼ carries Pack
        // Tactics; the mastiff deliberately doesn't, so the two
        // small-canine entries feel mechanically distinct rather than
        // overlapping. A future template-refactor that bolted Pack
        // Tactics onto the mastiff would erase the "guard dog vs
        // wild pack" contrast — the mastiff would just become a
        // weaker wolf.
        let a = make();
        assert!(!a.has_pack_tactics());
    }
}
