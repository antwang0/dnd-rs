use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::FORCE_EMPOWERED_REND;
use crate::actors::actor_template::CreatureTemplate;
use crate::actors::creatures::eldritch_cannons::{
    CONSTRUCT_CONDITION_IMMUNITIES, construct_damage_immunities,
};
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Steel Defender — the mechanical companion the Battle Smith Artificer
/// builds at subclass level 3 (TCE).
///
/// RAW: "Medium construct, AC 15, HP equal to 2 + your Intelligence
/// modifier + five times your artificer level, Speed 40 ft, immunity to
/// poison damage and to the charmed, exhausted, frightened, paralyzed,
/// petrified and poisoned conditions."
///
/// **The sturdiest thing on the feature-summon lane, and that is the
/// subclass.** The Ranger's companion is a wolf, the Wildfire spirit is
/// a positioning tool, the Fathomless tentacle cannot move. This is a
/// second front-line body: AC 15, forty-odd hit points, a 1d8 force
/// swing, and — the part that actually decides fights — immunity to the
/// poison and fear effects that take a wolf out of a fight without
/// killing it. A Battle Smith standing behind a defender is fielding
/// two combatants, which is why the artificer's own weapon is the
/// weakest of the four chassis: the rest of the subclass's martial
/// output went into this block.
///
/// Hit points are `8d8` ≈ 36 rather than RAW's level-scaled formula, for
/// the reason every summon in this engine carries a fixed pool — see
/// `SummonSpell::template`. Force typing on the rend is RAW and matters
/// for the same reason it does on the Force Ballista: nothing in the
/// bestiary resists it, so the defender's damage never turns off.
///
/// The construct envelope — poison and psychic immunity, the
/// mind-and-body condition set — is shared verbatim with the
/// Artillerist's cannons, and lives in `eldritch_cannons` because that
/// is where the three cannons that also carry it are written. Two
/// subclasses of one class arriving at the same defensive block is RAW
/// being consistent about what a construct is, and it should be one
/// list here for the same reason.
///
/// RAW's **Deflect Attack** reaction — impose disadvantage on an attack
/// against a creature within 5 ft of the defender — is left out. The
/// engine's reactive-disadvantage cohort is target-side (the *defended*
/// creature holds the tag and spends its own reaction), and a row whose
/// holder is a third party standing nearby would be a second lane for
/// one creature. What survives is the body and the swing, which is what
/// the feature is fielded for.
///
/// Glyph 'd' (lowercase) — for **d**efender, and distinct from the
/// artificer's own 'A' the way the Wildfire Spirit's 'w' is distinct
/// from its druid's 'W'.
pub static STEEL_DEFENDER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&FORCE_EMPOWERED_REND);
    CreatureTemplate {
        name: "Steel Defender",
        glyph: 'd',
        ac: 15,
        hitpoints: "8d8".parse().unwrap(),
        speed: 40.,
        // The rend is a STR swing off the defender's own sheet, which is
        // how the engine's summons roll — see `FORCE_EMPOWERED_REND`.
        // STR 14 lands the to-hit within a point of RAW's "proficiency
        // bonus + your Intelligence modifier".
        strength: 14,
        dexterity: 12,
        constitution: 14,
        intelligence: 4,
        wisdom: 10,
        charisma: 6,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        // RAW: "it understands the languages you speak" — the defender
        // takes orders and gives none.
        languages: HashSet::from([Language::Common]),
        cr: 1.0,
        size: Size::Medium,
        creature_type: CreatureType::Construct,
        actions,
        // RAW gives the defender proficiency in Dexterity and
        // Constitution saves, which on a construct that has to survive
        // area damage to be worth fielding is the half of the block
        // that keeps it standing through a Fireball.
        proficient_saves: HashSet::from([
            AbilityScoreType::Dexterity,
            AbilityScoreType::Constitution,
        ]),
        damage_modifiers: construct_damage_immunities(),
        condition_immunities: CONSTRUCT_CONDITION_IMMUNITIES.clone(),
        ..CreatureTemplate::defaults()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::conditions::Condition;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::{Coordinate, DamageModifier, DamageType};

    fn defender() -> ActorInstance {
        ActorInstance::from_creature_template(
            &STEEL_DEFENDER_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    /// The immunities are the reason the defender outlasts the other
    /// bodies on the feature-summon lane, so they are what the test
    /// pins — a defender that could be frightened off a front line
    /// would be a wolf with better armour.
    #[test]
    fn the_defender_shrugs_off_poison_and_fear() {
        let d = defender();
        assert_eq!(
            d.damage_modifier(DamageType::Poison),
            Some(DamageModifier::Immunity)
        );
        assert!(d.is_immune_to_condition(Condition::Frightened));
        assert!(d.is_immune_to_condition(Condition::Charmed));
    }

    /// Force is the point of the rend: it is the damage type nothing in
    /// the bestiary resists, so the defender's output never turns off.
    #[test]
    fn the_rend_deals_force() {
        assert_eq!(FORCE_EMPOWERED_REND.damage_type, DamageType::Force);
    }
}
