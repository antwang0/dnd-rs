use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::BLOOD_HAWK_BEAK;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, Skill};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Blood Hawk — CR ⅛ small beast. The Hawk's dangerous cousin, and the
/// roster's only carrier of SRD 5.2's *"or N damage if the target is
/// Bloodied"* clause.
///
/// Two lines make the stat block, and they compound:
///   - **Pack Tactics** — every beak rolls at advantage while another
///     hawk is adjacent to the target.
///   - **the beak's escalation** — 1d4 against a healthy target, 1d8
///     against a Bloodied one.
///
/// A flock is therefore not a linear threat. Against a full-strength
/// party it is a nuisance that rolls a lot of small dice at advantage;
/// the moment anybody drops below half, the same flock is rolling d8s
/// at advantage at the one creature least able to take them. Blood
/// hawks are what turns a fight somebody was already losing into a
/// fight that ends.
///
/// Action lane:
/// - **blood hawk beak** — DEX-based 1d4+DEX piercing, swapping to
///   1d8+DEX against a Bloodied target via
///   `SimpleWeapon::bloodied_dice`. A swap, not a rider — see the
///   field's docs for why that distinction is load-bearing.
///
/// Defensive identity: AC 12, ~7 HP (2d6). Fly 60 with no Flyby, so
/// unlike the owl the blood hawk *does* provoke when it leaves — it is
/// meant to dive in and stay in.
///
/// Stat shape: AC 12, ~7 HP, STR 6, DEX 14, CON 10, INT 3, WIS 14,
/// CHA 5. Speed 10 walking, fly 60. Skills Perception. Size Small.
/// CR ⅛. XP 25 per RAW.
pub static BLOOD_HAWK_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&BLOOD_HAWK_BEAK);
    CreatureTemplate {
        name: "Blood Hawk",
        // 'h' — beside the plain hawk's 'k' in the lowercase raptor
        // cohort; a distinct letter because the two are different
        // enough in a fight to be worth telling apart on the board.
        glyph: 'h',
        ac: 12,
        hitpoints: "2d6".parse().unwrap(),
        speed: 10.,
        fly_speed: 60.,
        strength: 6,
        intelligence: 3,
        dexterity: 14,
        wisdom: 14,
        constitution: 10,
        charisma: 5,
        skills: HashSet::from([Skill::Perception]),
        cr: 0.125,
        size: Size::Small,
        creature_type: CreatureType::Beast,
        actions,
        has_pack_tactics: true,
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
            &BLOOD_HAWK_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn blood_hawk_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.125);
        assert_eq!(a.size(), Size::Small);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("blood hawk beak").is_some());
        assert!(a.has_pack_tactics());
    }

    /// The beak carries a second, larger die for Bloodied targets.
    ///
    /// Pinned on the weapon rather than through a fight because the
    /// clause is data: if a future refactor of `SimpleWeapon` dropped
    /// the field, every blood hawk would keep working and would simply
    /// stop being a blood hawk.
    #[test]
    fn the_beak_escalates_against_a_wounded_target() {
        use crate::actions::monster_attacks::BLOOD_HAWK_BEAK;
        use crate::engine::dice::Dice;
        assert_eq!(BLOOD_HAWK_BEAK.damage_dice, Dice::new(1, 4));
        assert_eq!(BLOOD_HAWK_BEAK.bloodied_dice, Some(Dice::new(1, 8)));
    }
}
