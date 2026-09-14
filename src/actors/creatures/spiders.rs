use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::SPIDER_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, Skill, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Spider — CR 0 Tiny beast, one hit point, and the name the bestiary
/// had already spent.
///
/// `giant_spiders::GIANT_SPIDER_TEMPLATE` answered to "Spider" for a
/// long time, and its docstring records what that cost and names this
/// stat block as the thing it was not: *"SRD 5.2 has an actual **Spider**
/// — a Tiny CR 0 beast with one hit point and a bite that deals a single
/// point of piercing — and this is not remotely it."* This is it.
///
/// One hit point is the whole defensive profile, and it is not a
/// rounding of anything: the book prints `HP 1 (1d4 − 1)`, which is a
/// pool that can roll zero and is floored to one. What the spider has
/// instead of durability is a venom die worth more than twice its own
/// body — 2 (1d4) Poison behind 1 Piercing — and a Stealth bonus, which
/// on this engine's boards is a creature that is not seen until it is
/// already touching somebody.
///
/// Its two traits are the two the Giant Spider's docstring already
/// writes off, for the same reasons and in the same words. **Spider
/// Climb** wants a vertical axis the board has not got. **Web Walker**
/// waives a movement restriction the engine's Web zone already lets a
/// creature path around, and its second half — knowing where everything
/// else in the same web is — is a sense with no web to be in.
///
/// Stat shape per the SRD: AC 12, 1 HP (1d4−1), STR 2 / DEX 14 / CON 8 /
/// INT 1 / WIS 10 / CHA 2. Speed 20 (and a Climb 20 that is not
/// modeled). Darkvision 30. Skills: Stealth. CR 0.
pub static SPIDER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&SPIDER_BITE);
    CreatureTemplate {
        name: "Spider",
        // 'x' (lowercase) — the Giant Spider's 'X' one case down, which
        // is the relationship the two stat blocks actually have. Reading
        // a map and seeing both tells a player which one is about to
        // matter.
        glyph: 'x',
        ac: 12,
        // RAW's `1d4 - 1` averages 1.5 and can roll zero; the engine
        // floors a rolled pool at one hit point, which is also the
        // number the book prints. Written as `1d4-1` rather than as a
        // flat 1 so the ledger in `creatures::tests` can check it
        // against the book verbatim.
        hitpoints: "1d4-1".parse().unwrap(),
        speed: 20.,
        strength: 2,
        dexterity: 14,
        constitution: 8,
        intelligence: 1,
        wisdom: 10,
        charisma: 2,
        senses: HashSet::from([SpecialSense::Darkvision(30)]),
        skills: HashSet::from([Skill::Stealth]),
        cr: 0.0,
        size: Size::Tiny,
        creature_type: CreatureType::Beast,
        actions,
        ..CreatureTemplate::defaults()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::Coordinate;

    /// A `1d4 - 1` pool can roll zero, and a creature that arrives at
    /// zero hit points is one the engine has to not choke on. Swept
    /// across seeds rather than asserted once, because the interesting
    /// roll is the rare one — the same sweep the raven gets for the
    /// same reason.
    #[test]
    fn a_spider_always_arrives_with_at_least_one_hit_point() {
        for seed in 0..32u64 {
            let a = ActorInstance::from_creature_template(
                &SPIDER_TEMPLATE,
                Coordinate::new(0, 0),
                1,
                &mut FastRandRoller::with_seed(seed),
                0,
            )
            .unwrap();
            assert_eq!(a.cr(), 0.0);
            assert!(
                a.hitpoints() >= 1,
                "seed {}: a spider rolled {} hit points",
                seed,
                a.hitpoints()
            );
            assert!(a.find_action("spider bite").is_some());
        }
    }

    /// The bite is a bare 1 plus its venom, and the venom is the point.
    ///
    /// The spider's Dexterity is +2, so a chassis that folded the
    /// to-hit ability into the swing would print three piercing here
    /// rather than one — which is exactly what the flying snake was
    /// doing before `WeaponWithRider::flat_melee` existed. Asserted off
    /// the declaration rather than off a rolled swing because the claim
    /// is about the stat block: the swing half carries no modifier at
    /// all.
    #[test]
    fn a_spiders_puncture_is_the_bare_number_the_book_prints() {
        use crate::actions::monster_attacks::SPIDER_BITE;
        assert!(
            SPIDER_BITE.damage_ability.is_none(),
            "the spider's 1 piercing picked up an ability modifier"
        );
        assert_eq!(SPIDER_BITE.damage_dice.average_roll(), 1.0);
        assert_eq!(SPIDER_BITE.rider_dice, crate::engine::dice::Dice::new(1, 4));
    }
}
