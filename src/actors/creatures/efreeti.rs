use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{EFREETI_HURL_FLAME, EFREETI_MULTI, EFREETI_SCIMITAR};
use crate::actors::actor_template::CreatureTemplate;
use crate::actors::creatures::fire_elementals::{
    elemental_defaults,
};
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense,
};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Efreeti — CR 11 large elemental (fire genie). The proud and cruel
/// Sultan of the Plane of Fire: a horned brass-skinned giant wreathed
/// in continuous flame, wielding a massive scimitar that burns with the
/// hellish heat of the City of Brass. Sister to the Djinni (CR 11 air
/// genie) — same family of noble genies but on the fire side of the
/// elemental wheel.
///
/// Action lanes:
/// - **efreeti multiattack** — 2 scimitar swings per Action via the
///   shared homogeneous `Multiattack` chassis. Heavier per-swing dice
///   than the djinni's triple-scimitar (2d6+STR slashing + 2d6 fire
///   rider per swing); the fire rider routes through `add_flat_damage_
///   rider` so per-target fire resistance applies independently from
///   the slashing base.
/// - **efreeti scimitar** (standalone) — STR-based 2d6+STR slashing with
///   the 2d6 fire rider for the AI's single-target fallback.
/// - **efreeti hurl flame** — ranged CHA-attack fire bolt at 120ft range
///   (we cap at 30 tiles ≈ 75ft for the 40×20 maps). 5d6 fire on hit.
///   At-will (no recharge); the efreeti's "stay out of melee and lob
///   fireballs" stand-off lane. Same chassis as Horned Devil's Hurled
///   Flame but on the heftier 5d6 die.
///
/// Defensive identity: AC 17 (natural armor — the brass-skinned hide),
/// 200 HP (16d10+112). Heavier than the djinni's 161 HP (14d10+84) —
/// the efreeti's heat-forged body soaks more punishment. Standard
/// elemental envelope: non-magical BPS resistance, poison immunity.
/// Fire immunity (the efreeti IS heat). Magic Resistance gives
/// advantage on every save vs spells. Full elemental condition
/// envelope (Charmed / Frightened / Paralyzed / Petrified / Poisoned /
/// Asleep / Prone / Grappled / Restrained) via the shared
/// `ELEMENTAL_CONDITION_IMMUNITIES`.
///
/// Stat shape: AC 17, ~200 HP (16d10+112), STR 22, DEX 12, CON 24,
/// INT 16, WIS 15, CHA 16. Speed 40 (RAW also grants fly 60 which we
/// don't model). Senses: Darkvision 120ft. Languages: Ignan collapsed
/// to Primordial in this engine. Size Large. CR 11.
pub static EFREETI_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*EFREETI_MULTI);
    actions.push(&EFREETI_SCIMITAR);
    actions.push(&*EFREETI_HURL_FLAME);
    CreatureTemplate {
        name: "Efreeti",
        // 'F' (uppercase) — distinct from 'E' (Earth/Fire Elemental).
        // 'F' for the Flame-wreathed brass-skinned silhouette and frees
        // up 'E' for the four base elementals.
        glyph: 'F',
        ac: 17,
        // 16d10+112 ≈ 200 average per MM (CR 11).
        hitpoints: "16d10+112".parse().unwrap(),
        // RAW speed line: Speed 40 ft., fly 60 ft. (hover)
        speed: 40.0,
        fly_speed: 60.0,
        hovers: true,
        strength: 22,
        intelligence: 16,
        dexterity: 12,
        wisdom: 15,
        constitution: 24,
        charisma: 16,
        senses: HashSet::from([SpecialSense::Darkvision(120)]),
        languages: HashSet::from([Language::Primordial]),
        cr: 11.0,
        size: Size::Large,
        creature_type: CreatureType::Elemental,
        actions,
        // Efreeti proficient saves: INT, WIS, CHA per MM. The wise +
        // willful + force-of-personality saves; no DEX (the efreeti is
        // heavier and clumsier than the djinni's nimble winds).
        proficient_saves: HashSet::from([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]),
        // Standard elemental damage envelope (poison immune + BPS
        // resistance) overlaid with fire immunity — the efreeti's
        // signature elemental affinity.
        has_magic_resistance: true,
        has_extra_attack: true,
        ..elemental_defaults([(
            DamageType::Fire,
            DamageModifier::Immunity,
        )])
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
    fn efreeti_template_shape() {
        let a = ActorInstance::from_creature_template(
            &EFREETI_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 11.0);
        assert_eq!(a.size(), Size::Large);
        assert_eq!(a.creature_type(), CreatureType::Elemental);
        // Three action lanes — multi primary, scimitar standalone for AI
        // fallback, hurl flame for ranged stand-off.
        assert!(a.find_action("efreeti multiattack").is_some());
        assert!(a.find_action("efreeti scimitar").is_some());
        assert!(a.find_action("efreeti hurl flame").is_some());
    }

    #[test]
    fn efreeti_has_fire_elemental_envelope() {
        let a = ActorInstance::from_creature_template(
            &EFREETI_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        // Fire immunity is the signature trait — also poison immune,
        // BPS resistant from the shared elemental baseline.
        assert_eq!(
            a.damage_modifier(DamageType::Fire),
            Some(DamageModifier::Immunity)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Poison),
            Some(DamageModifier::Immunity)
        );
        assert_eq!(
            a.nonmagical_damage_modifier(DamageType::Slashing),
            Some(DamageModifier::Resistance)
        );
        assert!(a.has_magic_resistance());
        // Full elemental condition envelope.
        assert!(a.effectively_immune_to_condition(Condition::Frightened));
        assert!(a.effectively_immune_to_condition(Condition::Paralyzed));
        assert!(a.effectively_immune_to_condition(Condition::Poisoned));
    }
}
