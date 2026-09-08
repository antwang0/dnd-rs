use crate::actions::class_features::DRAKE_COMPANION_TAG;
use crate::engine::areas::AreaShape;
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{BreathWeapon, SimpleWeapon};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::dice::Dice;
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense,
};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// **Bite** — the drake companion's melee attack. RAW: "your spell
/// attack modifier to hit, reach 5 ft., one target. Hit: 1d6 +
/// proficiency bonus piercing damage."
///
/// The engine's summons roll off their own block rather than reaching
/// back to whoever called them — the same substitution the wildfire
/// spirit's Flame Seed makes, and for the same reason: a `SimpleWeapon`
/// has no channel to the summoner. STR 16 (+3) on the drake's block
/// lands the to-hit and the damage within a point of what a WIS-16
/// ranger's spell attack plus a +3 proficiency bonus would have given,
/// so the block-local roll costs the feature nothing at the table.
pub static DRAKE_BITE: SimpleWeapon = SimpleWeapon::melee(
    "bite",
    &["drake bite", "db"],
    AbilityScoreType::Strength,
    Dice::new(1, 6),
    DamageType::Piercing,
);

/// **Drake's Breath** — Drakewarden Ranger level-3 feature, resolved on
/// the drake's own turn. RAW: "the drake exhales a 15-foot cone of
/// damaging breath. Each creature in the cone must make a Dexterity
/// saving throw... taking 3d6 damage of the type chosen for the drake
/// on a failed save, or half as much damage on a successful one."
///
/// Radius 1 / range 2 is RAW's 15-foot cone under the same collapse
/// every dragon breath in the bestiary already takes — the adult
/// dragon's 60-foot cone ships as radius 4 / range 6, and this is that
/// envelope divided by the four the cones differ by. A square burst
/// rather than a cone for the reason `zones` gives about shapes: the
/// engine measures Chebyshev and nothing else, so a cone would need its
/// own geometry for a single consumer.
///
/// DC 14 is `8 + proficiency (+3) + the ranger's CON modifier (+2)` on
/// the level-9 half-caster chassis the Drakewarden template is built
/// to, which is what RAW's formula comes to there. Fixed on the drake's
/// block rather than read off the ranger for the same reason the bite's
/// numbers are.
///
/// **The recharge pool is the drake's, not the shared dragon one.** RAW
/// says "once you use this feature, you can't use it again until you
/// finish a short or long rest"; the engine's recharge lane is the
/// nearest cadence it has, and a private key keeps a Drakewarden
/// standing beside an actual dragon from either of them stealing the
/// other's breath.
pub static DRAKE_BREATH: BreathWeapon = BreathWeapon {
    display_name: "drake's breath",
    aliases: &["breath", "drake breath"],
    damage: Some((Dice::new(3, 6), DamageType::Fire)),
    save_ability: AbilityScoreType::Dexterity,
    dc: 14,
    shape: AreaShape::Cone { length: 6 },
    recharge_key: "drake_breath",
    condition: None,
    enemies_only: false,
};

/// Drake Companion — the Small dragon a Drakewarden Ranger calls to
/// their side, and the third creature in the bestiary (after the
/// wildfire spirit and the deep tentacle) that exists to be somebody's
/// subclass.
///
/// **It is the one of the three that can actually fight.** The wildfire
/// spirit is a position, the tentacle is a place; the drake is a body
/// with 30 hit points, AC 14, a bite and a cone of fire. That is the
/// deliberate difference between the Drakewarden and its two siblings —
/// a Fathomless warlock who loses the tentacle loses a bubble, and a
/// Drakewarden who loses the drake loses a combatant *and* the die on
/// every swing they make for the rest of the fight (see
/// `BOND_OF_FANG_AND_SCALE_TAG`).
///
/// Which makes the drake the only summon on the roster whose owner has
/// two reasons to keep it alive that pull in opposite directions. The
/// bond wants the drake within 30 ft of the ranger, and the breath
/// wants it standing in front of a crowd. A Drakewarden playing both is
/// steering the drake to the near edge of the enemy line rather than
/// into it.
///
/// Fire resistance rather than immunity: RAW gives the drake resistance
/// to the damage type chosen for it, and the engine's fire lane is wide
/// enough (every dragon, every fire elemental, half the fiends) that
/// immunity would have made the choice of element a balance decision
/// rather than a flavour one.
///
/// RAW's flight (speed 30, fly 30 from level 7) is left out for the
/// reason every other flier's is: the engine's map is one plane and a
/// creature that could ignore it would need a whole elevation axis.
/// RAW's level-15 Perfected Bond — the drake growing to Large, and the
/// ranger being able to ride it — is left out too, though the engine
/// has a mount lane that would carry the second half; the drake is
/// built at the level-9 chassis the ranger templates target, and Large
/// is not that drake.
///
/// Glyph 'k' (lowercase) — for dra**k**e, since 'd' is taken by the
/// bestiary's dragons and the pairing convention here is a lowercase
/// summon beside its uppercase summoner (the wildfire spirit's 'w', the
/// tentacle's 't'). The Drakewarden itself is 'D'.
pub static DRAKE_COMPANION_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&DRAKE_BITE);
    actions.push(&DRAKE_BREATH);
    CreatureTemplate {
        name: "Drake Companion",
        glyph: 'k',
        ac: 14,
        // RAW ties the drake's hit points to the ranger's level
        // (`5 + five times your ranger level`). A fixed roll is the
        // engine's convention for summons; 6d8 ≈ 27 sits where the
        // level-9 chassis the ranger templates target would put it.
        hitpoints: "6d8".parse().unwrap(),
        speed: 30.,
        strength: 16,
        dexterity: 12,
        constitution: 14,
        intelligence: 8,
        wisdom: 12,
        charisma: 10,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Draconic]),
        cr: 1.0,
        size: Size::Small,
        creature_type: CreatureType::Dragon,
        actions,
        damage_modifiers: HashMap::from([(DamageType::Fire, DamageModifier::Resistance)]),
        // The beacon Bond of Fang and Scale searches the board for.
        // Carried by the drake rather than back-linked from the ranger,
        // for the reasons `WILDFIRE_SPIRIT_TAG` gives.
        features: HashSet::from([DRAKE_COMPANION_TAG]),
        ..CreatureTemplate::defaults()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::Coordinate;

    fn drake() -> ActorInstance {
        ActorInstance::from_creature_template(
            &DRAKE_COMPANION_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    /// The whole point of the template carrying a feature set: the
    /// ranger's bond reads the board for this tag rather than holding a
    /// link that would dangle the first time the drake was re-summoned.
    #[test]
    fn the_drake_carries_the_beacon_the_bond_searches_for() {
        assert!(drake().has_passive_feature(DRAKE_COMPANION_TAG));
    }

    /// Both halves of the drake's contribution are on its own action
    /// list, because both measure from the drake and not from the
    /// ranger.
    #[test]
    fn the_drake_brings_its_own_bite_and_its_own_cone() {
        let names: Vec<&str> = DRAKE_COMPANION_TEMPLATE
            .actions
            .iter()
            .map(|a| a.name())
            .collect();
        assert!(names.contains(&"bite"), "{:?}", names);
        assert!(names.contains(&"drake's breath"), "{:?}", names);
    }

    /// The breath must not share the bestiary's `"breath_weapon"` pool:
    /// a Drakewarden fighting alongside — or against — a dragon would
    /// otherwise have one of the two spend the other's cone.
    #[test]
    fn the_breath_recharges_off_its_own_pool() {
        assert_ne!(DRAKE_BREATH.recharge_key, "breath_weapon");
    }
}
