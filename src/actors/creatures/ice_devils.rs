use crate::actions::class_features::MAGICAL_ATTACKS_TAG;
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{ICE_DEVIL_MULTI, ICE_DEVIL_SPEAR, ICE_DEVIL_TAIL};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size, Skill, SpecialSense,
};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Ice Devil — CR 14 large fiend, and the rung the devil ladder has
/// been missing: Bone Devil at 9, Horned Devil at 11, Erinyes at 12,
/// and then a five-point gap up to the Pit Fiend at 20.
///
/// It is the hierarchy's general, and the stat block reads like one —
/// 228 hit points, AC 18, three attacks a turn, and a defensive
/// envelope with no seam in it below the top tier:
///   - **Immune to cold, fire and poison**, which between them are most
///     of what a party brings to a devil fight.
///   - **Magic Resistance** on top of that.
///   - **Blindsight 120**, so nothing hides from it anywhere on the
///     board — no invisibility, no darkness, no illusion.
///
/// Action lanes:
/// - **ice devil multiattack** — two ice spears and the tail, per RAW's
///   "three Ice Spear attacks; it can replace one attack with a Tail
///   attack" with the substitution taken. Roughly eighty average damage
///   an Action, half of it cold that most things at this tier have no
///   answer to.
/// - **ice spear** / **ice devil tail** — the single swings, kept on the
///   list for the reaction lanes and the prompt.
///
/// Two RAW clauses are deliberately absent, both named at length on the
/// attacks themselves: the ice spear's ranged mode (`ICE_DEVIL_SPEAR`
/// explains why a kiting CR-14 boss with blindsight 120 is the wrong
/// reading) and its four-part debuff rider. **Ice Wall** (RAW: casts
/// *Wall of Ice* at level 8, Recharge 6) is the third: the engine's
/// spell lane prices a cast in slots, the devil has none, and a
/// recharge-gated slotless level-8 conjuration is a resource shape that
/// exists nowhere else here. `RechargingAttack` makes the gate cheap
/// now; what is still missing is a way to say "cast this without paying
/// for it", and inventing one for a single stat block is the wrong
/// order to do that in.
///
/// **Diabolical Restoration** is out of an encounter's scope, like every
/// other devil's.
///
/// Stat shape: AC 18, ~228 HP (24d10+96), STR 21, DEX 14, CON 18,
/// INT 18, WIS 15, CHA 18. Speed 40. Skills Insight, Perception,
/// Persuasion. Saves DEX, CON, WIS, CHA. Senses Blindsight 120.
/// Languages Infernal. Size Large. CR 14. XP 11,500 per RAW.
pub static ICE_DEVIL_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&ICE_DEVIL_SPEAR);
    actions.push(&ICE_DEVIL_TAIL);
    actions.push(&*ICE_DEVIL_MULTI);
    CreatureTemplate {
        name: "Ice Devil",
        // 'I' — free on the uppercase shelf; 'D' is the crowded dragon
        // and devil cohort and the ice devil deserves its own letter at
        // the tier it fights at.
        glyph: 'I',
        ac: 18,
        hitpoints: "24d10+96".parse().unwrap(),
        speed: 40.,
        strength: 21,
        intelligence: 18,
        dexterity: 14,
        wisdom: 15,
        constitution: 18,
        charisma: 18,
        skills: HashSet::from([Skill::Insight, Skill::Perception, Skill::Persuasion]),
        // Blindsight at the board's full width — the clause that makes
        // hiding from an ice devil a non-strategy.
        senses: HashSet::from([SpecialSense::Blindsight(120)]),
        languages: HashSet::from([Language::Infernal]),
        cr: 14.0,
        size: Size::Large,
        creature_type: CreatureType::Fiend,
        actions,
        damage_modifiers: HashMap::from([
            (DamageType::Cold, DamageModifier::Immunity),
            (DamageType::Fire, DamageModifier::Immunity),
            (DamageType::Poison, DamageModifier::Immunity),
        ]),
        condition_immunities: HashSet::from([Condition::Poisoned]),
        proficient_saves: HashSet::from([
            AbilityScoreType::Dexterity,
            AbilityScoreType::Constitution,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]),
        has_magic_resistance: true,
        // A greater devil's weapons are magical — which is what stops
        // two of them halving each other's damage for no reason RAW
        // recognises. See `MAGICAL_ATTACKS_TAG`.
        features: HashSet::from([MAGICAL_ATTACKS_TAG]),
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
            &ICE_DEVIL_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn ice_devil_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 14.0);
        assert_eq!(a.size(), Size::Large);
        assert_eq!(a.creature_type(), CreatureType::Fiend);
        assert!(a.find_action("ice devil multiattack").is_some());
        assert!(a.find_action("ice devil tail").is_some());
    }

    /// The envelope, which is what a CR-14 devil is buying with its
    /// challenge rating rather than the damage.
    #[test]
    fn the_ice_devil_has_no_elemental_seam() {
        let a = make();
        for dt in [DamageType::Cold, DamageType::Fire, DamageType::Poison] {
            assert_eq!(a.damage_modifier(dt), Some(DamageModifier::Immunity));
        }
        assert!(a.has_magic_resistance());
    }
}
