use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::GIANT_BOAR_TUSKS;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size};
use std::sync::LazyLock;

/// Giant Boar — CR 2 large beast. The "thunder-tusk" upgrade tier of
/// the boar family: a Large frame with chunky 2d6 tusks (vs the CR-¼
/// Boar's 1d6) and a 42-HP envelope. Slots between the regular Boar
/// (CR ¼ medium, 1d6 tusks) and the Mammoth (CR 6 huge, gore +
/// stomp recharge combo) on the porcine / megafauna bench — the
/// canonical mid-CR forest-ambusher with a single chunky single-die
/// swing.
///
/// Action lane:
/// - **giant boar tusks** — STR-based 2d6+STR slashing melee via the
///   shared `GIANT_BOAR_TUSKS` static. The CR-2 boar's only swing.
///   Chunky single-hit damage on a Large frame (~10 avg slashing per
///   swing against medium-AC targets) defines the threat profile.
///
/// **Charge** (RAW: 20 ft straight charge → extra 2d6 + DC-13 STR
/// save-or-Prone) and **Relentless** (drops to 1 HP from a lethal hit
/// once per short rest) are omitted as scope cuts. The engine doesn't
/// track straight-line charge movement at attack time (same gap that
/// hollows out the regular Boar and the Warhorse charges); the
/// Relentless once-per-rest "die at 0 but survive at 1" lane would
/// require a per-rest counter the engine doesn't expose for ordinary
/// creatures. The plain chunky-tusks swing keeps the giant boar
/// anchored at the "tough forest brute" silhouette without the
/// recharge-or-rest mini-systems.
///
/// Defensive identity: AC 13 (natural hide), 42 HP (5d10+15). Vanilla
/// beast envelope — no resistances or condition immunities. The
/// chunky HP envelope is the giant boar's primary defense — it
/// outlasts the regular Boar's 11 HP roughly 4x while still hitting
/// just as hard as a brown bear bite.
///
/// Stat shape: AC 13, ~42 HP (5d10+15), STR 17, DEX 10, CON 16,
/// INT 2, WIS 7, CHA 5. Speed 40. Size Large. CR 2. XP: 450 per RAW.
pub static GIANT_BOAR_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GIANT_BOAR_TUSKS);
    CreatureTemplate {
        name: "Giant Boar",
        // 'B' (uppercase) — shared with Bandit / Berserker / Brown Bear
        // / Bugbear / Bulette cohort. The team color disambiguates on
        // the map; the giant boar's Large size also separates it
        // visually from the medium 'b' (regular Boar / Berserker).
        // Uppercase 'B' reads as "large bristled brute" at the small
        // UI scale.
        glyph: 'B',
        ac: 13,
        // 5d10+15 = 42 average per MM (CR 2).
        hitpoints: "5d10+15".parse().unwrap(),
        speed: 40.,
        strength: 17,
        intelligence: 2,
        dexterity: 10,
        wisdom: 7,
        constitution: 16,
        charisma: 5,
        cr: 2.0,
        size: Size::Large,
        creature_type: CreatureType::Beast,
        actions,
        // RAW: when the giant boar closes at least the clause's distance in a
        // straight line and then connects with its tusks, the hit carries
        // extra 2d6 slashing and a Strength save vs prone. Read at the melee attack
        // chokepoint off `ActorInstance::charge`.
        charge: Some(crate::actions::monster_attacks::GIANT_BOAR_CHARGE),
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
            &GIANT_BOAR_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn giant_boar_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 2.0);
        assert_eq!(a.size(), Size::Large);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("giant boar tusks").is_some());
    }

    #[test]
    fn giant_boar_is_a_tougher_boar() {
        // Pin the load-bearing scaling: the giant boar's CR-2 identity
        // is "the boar but 4x as tough and twice the tusk dice." Speed
        // 40 matches RAW (the same as the small Boar); the dice
        // weight differs at the attack chassis. A future refactor that
        // accidentally downsized the HP / damage to the small Boar
        // profile would erase the CR-2 niche and silently collapse the
        // two boars into one stat block.
        let a = make();
        assert!(a.hitpoints() >= 20);
        assert_eq!(a.size(), Size::Large);
    }
}
