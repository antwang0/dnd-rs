use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{MERROW_BITE, MERROW_CLAWS, MERROW_HARPOON, MERROW_MULTI};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Merrow — CR 2 large aquatic ogre / corrupted merfolk. The deep-sea
/// raider counterpart to the Sahuagin (CR ½) and Lizardfolk (CR ½): one
/// tier up the humanoid ladder, traded into a Large frame with a
/// harpoon-and-bite multi. Pairs nicely with the existing aquatic pool
/// (Sahuagin, Water Elemental) and fills the CR 2 "boss humanoid" slot
/// alongside Bugbears and Cult Fanatics.
///
/// Stats roughly track MM Merrow at CR 2 — STR 18 (+4) drives the harpoon /
/// bite damage, CON 15 gives a solid HP pool (45 HP at 6d10+12), no
/// proficient saves. Senses include Darkvision 60.
///
/// Damage profile: no template-level resistances or immunities. The
/// merrow's identity is the reach-2 harpoon and the bite multi — it
/// hits hard but folds quickly under focused fire, matching the
/// "raider" archetype.
pub static MERROW_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&MERROW_BITE);
    actions.push(&MERROW_CLAWS);
    actions.push(&MERROW_HARPOON);
    actions.push(&*MERROW_MULTI);
    CreatureTemplate {
        name: "Merrow",
        // 'M' is reserved for Mind Flayer / Mage / Mariliths (uppercase
        // letter slot); use 'm' (lowercase) for the medium-CR merfolk.
        // 'm' was free and the wavy silhouette evokes the merrow's tail.
        glyph: 'm',
        ac: 13,
        // 6d10+12 = 45 average per MM.
        hitpoints: "6d10+12".parse().unwrap(),
        speed: 20., // 5e: 10ft walking, 40ft swim — we collapse to walking.
        strength: 18,
        intelligence: 8,
        dexterity: 10,
        wisdom: 10,
        constitution: 15,
        charisma: 9,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        // 5e Merrow: Abyssal (their corrupted-merfolk lineage) and
        // Primordial (covering Aquan in the consolidated Primordial slot
        // — the engine collapses Aquan / Auran / Ignan / Terran into a
        // single Primordial variant).
        languages: HashSet::from([Language::Abyssal, Language::Primordial]),
        cr: 2.0,
        size: Size::Large,
        creature_type: CreatureType::Humanoid,
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

    #[test]
    fn merrow_has_large_frame() {
        let a = ActorInstance::from_creature_template(
            &MERROW_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.size(), Size::Large);
    }

    #[test]
    fn merrow_carries_harpoon_and_multi() {
        let a = ActorInstance::from_creature_template(
            &MERROW_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert!(a.find_action("harpoon").is_some());
        assert!(a.find_action("harpoon + bite").is_some());
    }
}
