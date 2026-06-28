use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::AWAKENED_SHRUB_RAKE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Language, Size};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Awakened Shrub — CR 0 small plant. The Awaken spell's *sapling* tier:
/// a normal shrub given mobility and sentience. Sister to the Awakened
/// Tree (CR 2 huge plant) — same flavor envelope, much lower CR, much
/// lighter dice. Slots beside the Twig Blight (CR ⅛ small plant) on
/// the low-CR plant bench — the awakened shrub trades the twig
/// blight's Blindsight + venom envelope for the awakened-spell
/// envelope (one chosen language, the awakener's command).
///
/// Action lane:
/// - **shrub rake** — STR-based 1d4-1 slashing melee via the shared
///   `AWAKENED_SHRUB_RAKE` static. RAW: "+1 to hit, reach 5 ft, one
///   target. Hit: 1 (1d4-1) slashing damage." The engine's `max(1)`
///   damage gate keeps the per-swing yield at 1 most rolls; a crit
///   doubles the underlying 1d4-1 cleanly through the engine's
///   uniform crit chassis. Vanilla `SimpleWeapon` — no rider; the
///   shrub's load-bearing identity is "fragile plant scenery" rather
///   than dice output.
///
/// Defensive identity: AC 9 (small + low DEX, no natural armor), 10
/// HP (3d6). Vulnerable to fire (the canonical plant weakness shared
/// with Awakened Tree / Twig Blight / Needle Blight / Vine Blight).
/// Resistant to piercing (RAW: "Damage Resistance: piercing" — the
/// dense woody stems shrug off arrows and spear thrusts). RAW has no
/// bludgeoning resistance on the shrub (unlike the awakened tree,
/// whose bark grants BP-resistance), so a club / hammer cuts through
/// at full damage. No condition immunities — the awakened shrub is
/// still a "magically-animated plant," not an undead or construct,
/// so it can be frightened / charmed / restrained like any other
/// creature.
///
/// The MM-RAW "False Appearance" clause (indistinguishable from a
/// normal shrub while motionless) doesn't fit the engine's combat-
/// active chokepoint cleanly — every actor surfaces in
/// `combat_active`, so the stealth flavor is dropped (same scope cut
/// as the Awakened Tree).
///
/// Stat shape: AC 9, ~10 HP (3d6), STR 3, DEX 8, CON 11, INT 10,
/// WIS 10, CHA 6. Speed 20 (the lumbering walk — much slower than
/// the standard 30 because the awakened shrub roots-walks via slow
/// pseudopod-style branch movement). Languages: one chosen by the
/// awakener (we default to Common since the engine doesn't model the
/// awakener's language choice — same default as the Awakened Tree).
/// Size Small. CR 0. XP: 10 per RAW.
pub static AWAKENED_SHRUB_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&AWAKENED_SHRUB_RAKE);
    CreatureTemplate {
        name: "Awakened Shrub",
        // 's' (lowercase) — shared with Stirge / Slime / Sprite at the
        // tiny / small low-CR bench. The team color disambiguates on
        // the map and the awakened shrub's plant CR-0 context separates
        // it from the same-glyph cohort cleanly. Uppercase 'S' is
        // taken by Sahuagin / Skeleton / Specter / Spectator / Storm
        // Giant / Stone Giant cohort.
        glyph: 's',
        ac: 9,
        // 3d6 = 10.5 average per MM (CR 0).
        hitpoints: "3d6".parse().unwrap(),
        // Speed 20 — slower than the standard 30. Matches RAW and
        // mirrors the Awakened Tree's 20-ft lumber.
        speed: 20.,
        strength: 3,
        intelligence: 10,
        dexterity: 8,
        wisdom: 10,
        constitution: 11,
        charisma: 6,
        senses: HashSet::new(),
        languages: HashSet::from([Language::Common]),
        cr: 0.0,
        size: Size::Small,
        creature_type: CreatureType::Plant,
        actions,
        damage_modifiers: HashMap::from([
            // Pin the load-bearing plant resistance/vulnerability:
            // piercing-resistant (woody-stem RAW) and fire-vulnerable
            // (the canonical plant weakness). Unlike the Awakened Tree
            // — whose denser bark grants BP-resistance — the shrub's
            // lighter woody body only resists piercing per RAW.
            (DamageType::Piercing, DamageModifier::Resistance),
            (DamageType::Fire, DamageModifier::Vulnerability),
        ]),
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
            &AWAKENED_SHRUB_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn awakened_shrub_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.0);
        assert_eq!(a.size(), Size::Small);
        assert_eq!(a.creature_type(), CreatureType::Plant);
        assert!(a.find_action("shrub rake").is_some());
    }

    #[test]
    fn awakened_shrub_is_fire_vulnerable_and_piercing_resistant() {
        // Pin the load-bearing damage modifiers: fire-vulnerable (the
        // canonical plant weakness, shared with every other plant in
        // the pool — Awakened Tree, Twig Blight, Needle Blight, Vine
        // Blight) and piercing-resistant (RAW: dense woody stems shrug
        // off arrows and spear thrusts). Distinguishes the shrub from
        // the Awakened Tree, whose denser bark adds bludgeoning
        // resistance on top — a future refactor that copy-pasted the
        // tree's full BP+S triplet onto the shrub would over-tune the
        // CR-0 chassis's defensive profile.
        let a = make();
        assert_eq!(
            a.damage_modifier(DamageType::Fire),
            Some(DamageModifier::Vulnerability)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Piercing),
            Some(DamageModifier::Resistance)
        );
        // Bludgeoning explicitly NOT resisted — distinguishes from the
        // Awakened Tree's bark-resistance envelope.
        assert_eq!(a.damage_modifier(DamageType::Bludgeoning), None);
    }
}
