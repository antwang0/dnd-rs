use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{MARILITH_LONGSWORD, MARILITH_MULTI, MARILITH_TAIL};
use crate::actors::actor_template::{CreatureTemplate, damage_modifiers_from};
use crate::conditions::Condition;
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense,
};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Marilith — CR 16 demon. Six-armed snake-bodied general of the Abyss;
/// the apex of the multi-swing demon ladder. Signature lane:
/// - **Six longswords** (one per arm) — 2d8 + STR per swing.
/// - **One tail** (reach 2, 10ft) — 2d10 + STR bludgeoning per swing.
/// - **Multiattack** — all seven swings on one Action.
///
/// Slots between the Glabrezu (CR 9 mid-tier demon, 4-swing multi) and
/// the Balor (CR 19 apex). Magic Resistance is **on** — RAW gives it
/// and the template sets it. (This docstring spent some time saying
/// "magic-resistant in RAW but we don't model that yet", which was true
/// when it was written and had been false since `has_magic_resistance`
/// arrived; the flag two lines of code away was already set.) No
/// Legendary Resistance —
/// RAW: marilith doesn't have LR (that's reserved for the Balor / pit
/// fiend / Demon Lord tier in our pool).
pub static MARILITH_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&MARILITH_LONGSWORD);
    actions.push(&MARILITH_TAIL);
    actions.push(&*MARILITH_MULTI);
    CreatureTemplate {
        name: "Marilith",
        // 'Y' was free — uppercase letter to mark a CR-16 boss; visually
        // suggests the six-armed silhouette branching off the body.
        glyph: 'Y',
        ac: 18,
        // 19d10+85 ≈ 189 average per MM (CR 16 demon HP envelope).
        hitpoints: "19d10+85".parse().unwrap(),
        speed: 40.,
        strength: 18,
        dexterity: 20,
        constitution: 20,
        intelligence: 20,
        wisdom: 16,
        charisma: 20,
        senses: HashSet::from([SpecialSense::Truesight(120)]),
        languages: HashSet::from([Language::Abyssal]),
        cr: 16.0,
        size: Size::Large,
        creature_type: CreatureType::Fiend,
        actions,
        // Standard demon envelope: immune to poison; resistant to cold +
        // fire + lightning. Mirrors the Glabrezu / Balor damage profile
        // so radiant / force land cleanly on her.
        //
        // RAW's mundane B/P/S resistance is deliberately *not* in this
        // list: it is qualified to nonmagical attacks and so lives in
        // the `..CreatureTemplate::resistant_to_nonmagical_physical()`
        // tail, where the constructor's docstring explains why the
        // pairing has to be written in one place.
        damage_modifiers: damage_modifiers_from([
            (DamageType::Poison, DamageModifier::Immunity),
            (DamageType::Cold, DamageModifier::Resistance),
            (DamageType::Fire, DamageModifier::Resistance),
            (DamageType::Lightning, DamageModifier::Resistance),
        ]),
        // Marilith proficient saves: STR / CON / WIS / CHA per MM.
        proficient_saves: HashSet::from([
            AbilityScoreType::Strength,
            AbilityScoreType::Constitution,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]),
        // Demon condition envelope: Poisoned / Charmed / Frightened
        // immunity — same as the rest of the demon pool.
        condition_immunities: HashSet::from([
            Condition::Poisoned,
            Condition::Charmed,
            Condition::Frightened,
        ]),
        has_magic_resistance: true,
        has_extra_attack: true,
        // 5e **Magic Weapons**: "the marilith's weapon attacks are magical."
        features: HashSet::from([crate::actions::class_features::MAGICAL_ATTACKS_TAG]),
        ..CreatureTemplate::resistant_to_nonmagical_physical()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::encounter::EncounterInstance;
    use crate::engine::types::Coordinate;

    fn arena() -> EncounterInstance {
        use crate::engine::actor_gen::ActorGenParams;
        use crate::engine::terrain_gen::TerrainGenParams;
        let tp = TerrainGenParams {
            width: 30,
            height: 20,
            branch_depth: 0,
            branch_prob: 0.0,
        };
        let ap = ActorGenParams {
            cr_target: 0.0,
            n_teams: 0,
            pc_template: None,
            start_team: 0,
        };
        EncounterInstance::from_params(&tp, &ap, Some(1)).unwrap()
    }

    /// Marilith template carries the demon envelope (poison immunity,
    /// cold/fire/lightning/B/P/S resistance, charmed/frightened/poisoned
    /// condition immunities) and exposes the seven-swing multi.
    #[test]
    fn marilith_template_carries_demon_envelope() {
        let mut e = arena();
        let id = e
            .instantiate_creature(&MARILITH_TEMPLATE, Coordinate::new(5, 5), 1, 0)
            .unwrap();
        let m = &e.actors[&id];
        assert!(m.is_immune_to(DamageType::Poison));
        assert!(m.is_resistant_to(DamageType::Cold));
        assert!(m.is_resistant_to(DamageType::Fire));
        assert!(m.is_resistant_to(DamageType::Lightning));
        assert!(m.resists_nonmagical(DamageType::Bludgeoning));
        assert!(m.is_immune_to_condition(Condition::Poisoned));
        assert!(m.is_immune_to_condition(Condition::Charmed));
        assert!(m.is_immune_to_condition(Condition::Frightened));
        // No LR — marilith RAW does not include Legendary Resistance.
        assert_eq!(m.legendary_resistance_max(), 0);
        assert!(m.find_action("marilith longsword").is_some());
        assert!(m.find_action("marilith tail").is_some());
        assert!(m.find_action("marilith multiattack").is_some());
    }
}
