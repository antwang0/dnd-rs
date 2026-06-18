use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{DRYAD_CLUB, DRYAD_FEY_CHARM};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Dryad — CR 1 medium fey. The classic forest-spirit charmer: a low-HP
/// caster-flavored monster whose signature `DRYAD_FEY_CHARM` action
/// targets a single enemy at 12-tile (30 ft) range with a WIS DC-14
/// save vs Charmed (10-round duration). Pairs with a vanilla 1d4 club
/// for melee fallback when the charm misses or its target is already
/// charm-immune.
///
/// Templates: AC 11 (barkskin-flavored natural armor), 22 HP (5d8),
/// STR 10, DEX 12, CON 11, INT 14, WIS 15, CHA 18 (primary save DC stat).
/// Languages: Elvish, Sylvan. Senses: Darkvision 60 ft. Fits the
/// low-CR fey gap between the harpy (CR 1) and the green hag (CR 3) on
/// the fey-controller ladder — Luring Song's AoE charm at the harpy
/// tier, the dryad's single-target lock at CR 1, the hag's claws +
/// resistance envelope at CR 3.
pub static DRYAD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&DRYAD_CLUB);
    actions.push(&*DRYAD_FEY_CHARM);
    CreatureTemplate {
        name: "Dryad",
        // 'r' for dryad — lowercase tree-spirit glyph. Distinct from 'D'
        // (Druid / Dragon family) and 'd' (Medusa).
        glyph: 'r',
        ac: 11,
        hitpoints: "5d8".parse().unwrap(),
        speed: 30.,
        strength: 10,
        intelligence: 14,
        dexterity: 12,
        wisdom: 15,
        constitution: 11,
        charisma: 18,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Elvish, Language::Sylvan]),
        cr: 1.0,
        size: Size::Medium,
        creature_type: CreatureType::Fey,
        actions,
        // Dryads have Magic Resistance per MM: advantage on saving
        // throws against spells. Slots into the standard caster-counter
        // lane alongside green hag / pseudodragon / lich.
        has_magic_resistance: true,
        // Fey Ancestry: advantage on saves against being Charmed, and
        // magic can't put them to sleep. We use the existing
        // `has_fey_ancestry` flag which gates Charmed / Asleep immunity
        // at the `dynamic_immunity_to` chokepoint.
        has_fey_ancestry: true,
        ..CreatureTemplate::defaults()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::conditions::Condition;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::Coordinate;

    #[test]
    fn dryad_has_fey_ancestry_charm_immunity() {
        let a = ActorInstance::from_creature_template(
            &DRYAD_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        // Fey ancestry approximates as Charmed / Asleep immunity in this
        // engine — confirms a dryad can't be charmed by another dryad.
        assert!(a.effectively_immune_to_condition(Condition::Charmed));
    }

    #[test]
    fn dryad_has_magic_resistance() {
        let a = ActorInstance::from_creature_template(
            &DRYAD_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert!(a.has_magic_resistance());
    }
}
