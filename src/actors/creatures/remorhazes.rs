use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{REMORHAZ_BITE, SWALLOW_BONUS};
use crate::actors::actor_template::{CreatureTemplate, damage_modifiers_from};
use crate::engine::attack::{MeleeReflect, ReflectDamage};
use crate::engine::dice::Dice;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Remorhaz **Heated Body** — RAW: "A creature that touches the remorhaz
/// or hits it with a melee attack while within 5 feet of it takes 10
/// (3d6) fire damage."
///
/// The largest reflect in the bestiary by a wide margin — three times
/// the salamander's die and three times the azer's — and it is the
/// clause that makes closing on a remorhaz a decision rather than a
/// default. Ten average fire a swing, on top of a body with a hundred
/// and ninety-five hit points, means a melee party pays for the whole
/// fight twice.
pub static REMORHAZ_HEATED_BODY: MeleeReflect = MeleeReflect {
    damage: ReflectDamage::Dice(Dice::new(3, 6)),
    damage_type: DamageType::Fire,
    label: "heated body",
};

/// Remorhaz — CR 11 huge monstrosity. A hundred-foot arctic centipede
/// that runs hot enough to melt the ice it burrows through, and the
/// bestiary's purest expression of "do not be next to this".
///
/// Action lane: **remorhaz bite**, 6d10+STR piercing plus 3d6 fire at
/// reach 10 ft. Forty-seven average damage from one swing, which is the
/// single hardest hit in the pool below the ancient dragons, and it is
/// two damage types rather than one so a target resistant to piercing
/// still takes the fire.
///
/// The defensive envelope is the reflect above plus **fire and cold
/// immunity** — the first because it is a furnace and the second
/// because it lives in a glacier. Between the two, a party that packed
/// for the arctic has brought exactly the wrong spells: cold does
/// nothing, and fire does nothing, and every round spent in contact
/// costs 3d6 of the second one back.
///
/// RAW's **Swallow** is carried, and it is the one stomach in the book
/// that does two things at once: 3d6 acid *plus* 3d6 fire at the start
/// of each of the remorhaz's turns, on up to two creatures. Which means
/// the fire immunity above cuts both ways — the remorhaz is the monster
/// you meet in the one environment where everybody brought fire
/// resistance, and inside it that resistance halves the tick. See
/// `REMORHAZ_SWALLOW`.
///
/// Stat shape per the SRD: AC 17 (natural armor), 195 HP (17d12+85),
/// STR 24 / DEX 13 / CON 21 / INT 4 / WIS 10 / CHA 5. Speed 40 (plus a
/// 20 ft burrow RAW, which the engine has no vertical axis for).
/// Darkvision 60, Tremorsense 60 — the sense that matters, since a
/// remorhaz hunts through solid ice and does not need to see. CR 11.
pub static REMORHAZ_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&REMORHAZ_BITE);
    actions.push(&SWALLOW_BONUS);
    CreatureTemplate {
        name: "Remorhaz",
        // 'R' (uppercase) — shared with the Roper and the Rogue at CRs
        // five and ten rungs away.
        glyph: 'R',
        ac: 17,
        // 17d12+85 ≈ 195 average per the SRD (CR 11).
        hitpoints: "17d12+85".parse().unwrap(),
        speed: 40.,
        strength: 24,
        dexterity: 13,
        constitution: 21,
        intelligence: 4,
        wisdom: 10,
        charisma: 5,
        senses: HashSet::from([SpecialSense::Darkvision(60), SpecialSense::Tremorsense(60)]),
        cr: 11.0,
        size: Size::Huge,
        creature_type: CreatureType::Monstrosity,
        actions,
        damage_modifiers: damage_modifiers_from([
            (DamageType::Cold, DamageModifier::Immunity),
            (DamageType::Fire, DamageModifier::Immunity),
        ]),
        natural_melee_reflect: Some(REMORHAZ_HEATED_BODY),
        swallow: Some(&crate::actions::monster_attacks::REMORHAZ_SWALLOW),
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
            &REMORHAZ_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn remorhaz_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 11.0);
        assert_eq!(a.size(), Size::Huge);
        assert_eq!(a.creature_type(), CreatureType::Monstrosity);
        assert!(a.find_action("remorhaz bite").is_some());
    }

    /// A furnace living in a glacier is immune to both, which is what
    /// makes it a bad fight for anybody who packed for the arctic. Both
    /// immunities and the reflect asserted together, because each alone
    /// reads as an ordinary elemental creature.
    #[test]
    fn the_remorhaz_answers_ice_and_fire_with_more_fire() {
        let a = make();
        assert_eq!(
            a.damage_modifier(DamageType::Cold),
            Some(DamageModifier::Immunity)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Fire),
            Some(DamageModifier::Immunity)
        );
        let reflect = a.natural_melee_reflect().expect("heated body");
        assert_eq!(reflect.damage_type, DamageType::Fire);
    }

    /// The bite is two damage types at reach 10, and both halves are
    /// load-bearing: a piercing-resistant target still takes the fire,
    /// and a reach-5 bite would let the remorhaz be fought from a tile
    /// its reflect cannot reach.
    #[test]
    fn the_bite_carries_fire_past_the_reach_of_anything_it_bites() {
        use crate::actions::action_template::Action;
        let types = REMORHAZ_BITE.damage_types();
        assert!(types.contains(&DamageType::Piercing));
        assert!(types.contains(&DamageType::Fire));
        assert_eq!(REMORHAZ_BITE.reach, 2);
    }
}
