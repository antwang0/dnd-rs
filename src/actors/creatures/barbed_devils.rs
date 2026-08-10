use crate::actions::class_features::{DEVILS_SIGHT_TAG, MAGICAL_ATTACKS_TAG};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    BARBED_DEVIL_CLAW, BARBED_DEVIL_HURL_FLAME, BARBED_DEVIL_MULTI, BARBED_DEVIL_TAIL,
};
use crate::actors::actor_template::{CreatureTemplate, damage_modifiers_from};
use crate::conditions::Condition;
use crate::engine::attack::{MeleeReflect, ReflectDamage};
use crate::engine::dice::Dice;
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense,
};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Barbed Devil **Barbed Hide** — RAW: "At the start of each of its
/// turns, the barbed devil deals 5 (1d10) piercing damage to any
/// creature grappling it."
///
/// Re-aimed from the grappler to the melee attacker, which is the one
/// place this stat block departs from RAW and does so deliberately.
/// The engine has no per-turn "who is holding me" tick, and a clause
/// scoped to grapplers would fire for perhaps one creature in the
/// bestiary; the same spikes that punish a grab punish a fist, and the
/// natural-melee-reflect lane is where every other body-is-a-hazard
/// creature already lives. Piercing rather than the fire the hamatula's
/// other lane deals, because the hide is spikes and not flame — which
/// matters against the devils and demons that resist fire and not
/// physical damage.
pub static BARBED_DEVIL_BARBED_HIDE: MeleeReflect = MeleeReflect {
    damage: ReflectDamage::Dice(Dice::new(1, 10)),
    damage_type: DamageType::Piercing,
    label: "barbed hide",
};

/// Barbed Devil (Hamatula) — CR 5 medium fiend. The Nine Hells' jailer:
/// a devil covered head to foot in iron thorns that fights at whatever
/// range the party is not comfortable at.
///
/// Action lanes:
/// - **barbed devil multiattack** — 2 claws + 1 tail. Three swings,
///   all piercing, so a target that resists one resists all of them —
///   which is the hamatula's weakness and the reason it also throws
///   fire.
/// - **barbed devil claw / tail** (standalone) — the AI's fallbacks
///   when the multi cannot be afforded.
/// - **hurl flame** — 3d6 fire at sixteen tiles of clean band. Filed as
///   a weapon rather than a spell; see the attack's own docs for why.
///
/// Defensive identity is the standard devil envelope — fire and poison
/// immune, cold resistant, Magic Resistance, Devil's Sight, magical
/// weapon attacks — plus the reflect above, which is what makes closing
/// on a hamatula cost something.
///
/// Stat shape per the SRD: AC 15 (natural armor), 110 HP (13d8+52), STR
/// 16 / DEX 17 / CON 18 / INT 12 / WIS 14 / CHA 14. Speed 30. Proficient
/// STR / CON / WIS / CHA saves — RAW's devil save spread, and the reason
/// a hamatula is a poor target for almost every save-or-suck the party
/// owns. Darkvision 120. Languages: Infernal. CR 5.
pub static BARBED_DEVIL_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*BARBED_DEVIL_MULTI);
    actions.push(&BARBED_DEVIL_CLAW);
    actions.push(&BARBED_DEVIL_TAIL);
    actions.push(&BARBED_DEVIL_HURL_FLAME);
    CreatureTemplate {
        name: "Barbed Devil",
        // 'B' (uppercase) — the devil band, shared with the Bearded
        // Devil two CRs below and the Balor fourteen above.
        glyph: 'B',
        ac: 15,
        // 13d8+52 ≈ 110 average per the SRD (CR 5).
        hitpoints: "13d8+52".parse().unwrap(),
        speed: 30.,
        strength: 16,
        dexterity: 17,
        constitution: 18,
        intelligence: 12,
        wisdom: 14,
        charisma: 14,
        senses: HashSet::from([SpecialSense::Darkvision(120)]),
        languages: HashSet::from([Language::Infernal]),
        cr: 5.0,
        size: Size::Medium,
        creature_type: CreatureType::Fiend,
        actions,
        damage_modifiers: damage_modifiers_from([
            (DamageType::Cold, DamageModifier::Resistance),
            (DamageType::Fire, DamageModifier::Immunity),
            (DamageType::Poison, DamageModifier::Immunity),
        ]),
        condition_immunities: HashSet::from([Condition::Poisoned]),
        proficient_saves: HashSet::from([
            AbilityScoreType::Strength,
            AbilityScoreType::Constitution,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]),
        has_magic_resistance: true,
        features: HashSet::from([DEVILS_SIGHT_TAG, MAGICAL_ATTACKS_TAG]),
        natural_melee_reflect: Some(BARBED_DEVIL_BARBED_HIDE),
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
            &BARBED_DEVIL_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn barbed_devil_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 5.0);
        assert_eq!(a.creature_type(), CreatureType::Fiend);
        assert!(a.find_action("barbed devil multiattack").is_some());
        assert!(a.find_action("barbed devil claw").is_some());
        assert!(a.find_action("barbed devil tail").is_some());
        assert!(a.find_action("hurl flame").is_some());
    }

    /// The hamatula's two ranges. Three piercing swings in contact and
    /// a fire lane at distance is not decoration: a target that resists
    /// piercing turns the multiattack off entirely, and the flame is
    /// the reason the devil is still a CR 5 against it.
    #[test]
    fn the_hamatula_threatens_at_both_ranges_and_in_two_damage_types() {
        let a = make();
        let claw = a.find_action("barbed devil claw").expect("claw");
        assert!(claw.is_melee_attack());
        assert!(claw.damage_types().contains(&DamageType::Piercing));

        let flame = a.find_action("hurl flame").expect("hurl flame");
        assert!(!flame.is_melee_attack());
        assert!(flame.damage_types().contains(&DamageType::Fire));
        // A ranged weapon has to declare the band the long-range and
        // underwater rules both read.
        assert_eq!(flame.normal_range(), Some(16));
    }

    /// Barbed Hide is the clause that makes closing cost something, and
    /// it is piercing on purpose — a fire-typed reflect would do nothing
    /// to the other devils a hamatula most often fights beside.
    #[test]
    fn the_barbed_hide_bites_back_in_a_type_its_own_kin_do_not_resist() {
        let a = make();
        let reflect = a.natural_melee_reflect().expect("barbed hide");
        assert_eq!(reflect.damage_type, DamageType::Piercing);
        assert_eq!(
            a.damage_modifier(DamageType::Fire),
            Some(DamageModifier::Immunity),
            "…and the devil itself is immune to the type it does not reflect"
        );
    }
}
