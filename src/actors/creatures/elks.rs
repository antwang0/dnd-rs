use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{ELK_HOOVES, ELK_RAM};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size};
use std::sync::LazyLock;

/// Elk — CR ¼ large beast. The forest cervid: a fast (50 ft)
/// Large frame with two distinct swings (Ram or Hooves per
/// Action, no Multiattack per RAW). Slots between the Riding
/// Horse / Draft Horse (CR ¼ large 2d4 hooves) and the Boar
/// (CR ¼ medium 1d6 tusks) on the CR-¼ herbivore-megafauna
/// bench — the canonical "wilderness ungulate" silhouette.
/// Distinguished from the Giant Elk (CR 2 huge) by being smaller
/// and lacking Multiattack; distinguished from the horse cohort
/// by carrying a two-action lane (Ram + Hooves) instead of a
/// single hooves swing.
///
/// Action lane:
/// - **elk ram** — STR-based 1d6+STR bludgeoning melee via the
///   shared `ELK_RAM` static. The lower-dice / harder-hit option
///   (1d6 = 3.5 + STR 16 = +3, ~6.5 avg). RAW pairs this with
///   the Charge rider (extra 2d6 + DC-13 STR vs Prone after
///   20 ft straight-line dash); the straight-line gate is the
///   same scope cut that hollows the Boar / Warhorse charges.
/// - **elk hooves** — STR-based 2d4+STR bludgeoning melee via
///   the shared `ELK_HOOVES` static. The higher-average swing
///   (2d4 = 5 + STR 16 = +3, ~8 avg). RAW restricts this to
///   prone-only targets; the engine drops the restriction so the
///   AI can pick either swing per turn (see `ELK_HOOVES`'s
///   docstring for the reasoning).
///
/// The elk lacks Multiattack — both swings sit as standalone
/// Actions, and the AI picks one per turn. The dual-action lane
/// is distinct from the horse cohort's single hooves option and
/// from the boar's single tusks option, giving the elk a
/// slightly richer tactical surface at the same CR.
///
/// Defensive identity: AC 10 (no natural hide, no armor), 13 HP
/// (2d10+2). Vanilla beast envelope — no resistances or
/// condition immunities. Speed 50 is the load-bearing tactical
/// stat — faster than every other CR-¼ beast (the horse cohort
/// caps at 60 only on the Warhorse; the Boar / Goat / Camel sit
/// at 40), letting the elk dictate engagement range.
///
/// Stat shape: AC 10, ~13 HP (2d10+2), STR 16, DEX 10, CON 12,
/// INT 2, WIS 10, CHA 6. Speed 50. Size Large. CR ¼. XP: 50
/// per RAW.
pub static ELK_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&ELK_RAM);
    actions.push(&ELK_HOOVES);
    CreatureTemplate {
        name: "Elk",
        // 'e' (lowercase) — shared with Ettin / Erinyes / Efreeti
        // cohort (uppercase). The team color disambiguates on the
        // map; the elk's beast / herbivore context separates it
        // from the upper-tier fiends / giants at the prompt layer.
        // Uppercase 'E' is taken by the larger humanoid / fiend
        // bench; lowercase 'e' reads as "large ambient ungulate"
        // beside 'b' (Boar), 'h' (Horse).
        glyph: 'e',
        ac: 10,
        // 2d10+2 = 13 average per MM (CR ¼).
        hitpoints: "2d10+2".parse().unwrap(),
        speed: 50.,
        strength: 16,
        intelligence: 2,
        dexterity: 10,
        wisdom: 10,
        constitution: 12,
        charisma: 6,
        cr: 0.25,
        size: Size::Large,
        // 5e Mounted Combat: MM: "an elk can serve as a mount", and the Elk totem barbarian's whole
        // idea is the thing it is named after.
        mountable: true,
        creature_type: CreatureType::Beast,
        actions,
        // RAW: when the elk closes at least the clause's distance in a
        // straight line and then connects with its ram, the hit carries
        // extra 2d6 bludgeoning and a Strength save vs prone. Read at the melee attack
        // chokepoint off `ActorInstance::charge`.
        charge: Some(crate::actions::monster_attacks::ELK_CHARGE),
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
            &ELK_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn elk_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.25);
        assert_eq!(a.size(), Size::Large);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("elk ram").is_some());
        assert!(a.find_action("elk hooves").is_some());
    }

    #[test]
    fn elk_carries_dual_action_lane() {
        // Pin the load-bearing tactical distinction at this CR:
        // the elk has TWO standalone swings (Ram + Hooves) rather
        // than one swing or one multiattack. A future refactor
        // that quietly folded them into a single Multiattack /
        // CompoundAttack would erase the "pick one per turn"
        // surface that distinguishes the elk from the horse /
        // boar / goat cohort at CR ¼.
        let a = make();
        assert!(a.find_action("elk ram").is_some());
        assert!(a.find_action("elk hooves").is_some());
    }

    #[test]
    fn elk_is_the_fastest_cr_quarter_beast() {
        // Pin the load-bearing tactical stat: speed 50 is faster
        // than every other CR-¼ beast (Boar 40, Goat 40, Camel 50,
        // Riding Horse 60 — but the riding horse is the lone
        // outlier). A future refactor that quietly dropped speed
        // would erase the "skirmishing ungulate" identity.
        let a = make();
        assert!(a.speed() >= 50.0);
    }
}
