use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    GORGON_GORE, GORGON_HOOVES, GORGON_MULTI, GORGON_PETRIFYING_BREATH,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Gorgon — CR 5 large monstrosity. A mechanical-looking iron bull whose
/// signature trick is its petrifying-breath cone — a CON-save AoE that
/// applies the `Petrified` condition for one round to every failed-save
/// victim. Pairs with a heavy gore + hooves multi (2d12 piercing +
/// 2d10 bludgeoning, both STR-modded) for a brutal melee envelope and a
/// 40ft walk speed for chase pressure.
///
/// Templates: AC 19 (iron hide), 114 HP (12d10+48), STR 20 (+5 mod),
/// DEX 11, CON 18 (+4), INT 2, WIS 12, CHA 7. Construct-like in MM RAW
/// but classified as a monstrosity; we keep monstrosity for the
/// creature_type since it doesn't share the construct condition-immunity
/// envelope (gorgons are flesh-and-iron, not pure machine).
///
/// Recharge: petrifying breath uses the shared `"breath_weapon"` key so
/// the start-of-turn d6 roll flips it back on a 5-6 — same pool the
/// dragons / behir / winter wolf share.
pub static GORGON_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GORGON_GORE);
    actions.push(&GORGON_HOOVES);
    actions.push(&*GORGON_PETRIFYING_BREATH);
    actions.push(&*GORGON_MULTI);
    CreatureTemplate {
        name: "Gorgon",
        // 'G' for Gorgon — capital because Large.
        glyph: 'G',
        ac: 19,
        // 12d10+48 ≈ 114 average per MM CR 5.
        hitpoints: "12d10+48".parse().unwrap(),
        speed: 40.,
        strength: 20,
        intelligence: 2,
        dexterity: 11,
        wisdom: 12,
        constitution: 18,
        charisma: 7,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        cr: 5.0,
        size: Size::Large,
        creature_type: CreatureType::Monstrosity,
        actions,
        // Gorgons resist nothing in MM but are immune to Petrified
        // themselves (their own breath can't stone-lock them — a
        // tactical safety so a friendly-fire breath doesn't trap two
        // gorgons in adjacent tiles).
        condition_immunities: HashSet::from([Condition::Petrified]),
        // Petrifying breath gates on a d6 of 5-6 at the start of the
        // gorgon's turn — shared `"breath_weapon"` pool with the dragons.
        recharge_abilities: vec![("breath_weapon", 5)],
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
    fn gorgon_is_petrified_immune() {
        let a = ActorInstance::from_creature_template(
            &GORGON_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert!(a.is_immune_to_condition(Condition::Petrified));
    }
}
