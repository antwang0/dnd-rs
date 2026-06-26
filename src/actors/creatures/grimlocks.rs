use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::GRIMLOCK_SPIKED_CLUB;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Grimlock — CR ¼ humanoid blind savage. The "Underdark cave dweller"
/// tier of humanoid: a lightless-warren native that hunts entirely by
/// sound, smell, and the tremorsense-style blindsight RAW grants the
/// species. Wields a crude spiked bone club that does bludgeoning +
/// piercing in one swing. Slots between the Goblin (CR ¼, scimitar +
/// shortbow) and the Orc (CR ½, greataxe + javelin) on the humanoid
/// raider bench — distinctive as the *blind hunter* lane in a roster
/// otherwise built on Darkvision-and-line-of-sight humanoids.
///
/// Action lane:
/// - **spiked bone club** — STR-based 1d4+STR bludgeoning + 1d4
///   piercing rider via the shared `WeaponWithRider::melee` chassis
///   (`GRIMLOCK_SPIKED_CLUB`). The split typing matters for
///   resistance-aware targets: a fully bludgeoning-resistant skeleton
///   still eats the spike rider at full value, and a piercing-
///   resistant chitin-shell creature still eats the bone-shaft base.
///
/// **Blindsight 30 ft (blind beyond this radius)** — RAW: "The
/// grimlock can perceive its surroundings without relying on sight,
/// within a specific radius." Routes through the standard
/// `SpecialSense::Blindsight(30)` slot — the same chokepoint the
/// engine reads when computing whether a target is concealed by
/// magical darkness, invisibility, or fog. The "blind beyond this
/// radius" half is *not* yet a flag the engine tracks (the grimlock
/// has no eyes — Invisibility *outside* its 30ft is irrelevant
/// because it can't see beyond that anyway, and *inside* the 30ft
/// Blindsight cancels the concealment). The mechanical effect is
/// the standard "Blindsight 30 ft" — perceive everything within
/// melee/short range regardless of light or concealment.
///
/// **Stone Camouflage** (advantage on Stealth in rocky terrain) is
/// RAW flavor-only — the engine doesn't surface skill checks
/// through combat.
///
/// Defensive identity: AC 11 (hide armor), 11 HP (2d8+2). Vanilla
/// humanoid envelope — no resistances or condition immunities. The
/// grimlock dies to a single solid hit; its threat lives in the
/// dual-typed club rider (bypasses most single-type resistances) and
/// the Blindsight (immune to Invisibility / Darkness shenanigans
/// inside its perception radius).
///
/// Stat shape: AC 11, ~11 HP (2d8+2), STR 16, DEX 12, CON 12, INT 9,
/// WIS 8, CHA 6. Speed 30. Senses: Blindsight 30. Languages:
/// Undercommon (the Underdark trade language). Size Medium. CR ¼.
/// XP: 50 per RAW.
pub static GRIMLOCK_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GRIMLOCK_SPIKED_CLUB);
    CreatureTemplate {
        name: "Grimlock",
        // 'k' (lowercase) — distinct cave-dweller mnemonic, mirroring
        // 'k' (Kobold) at the same CR-¼ humanoid tier. The team color
        // disambiguates Kobold vs Grimlock on the map. Uppercase 'K'
        // is taken by Knight / Kraken; lowercase 'k' shares the
        // Underdark / cave-raider niche cleanly.
        glyph: 'k',
        ac: 11,
        // 2d8+2 ≈ 11 average per MM (CR ¼).
        hitpoints: "2d8+2".parse().unwrap(),
        speed: 30.,
        strength: 16,
        intelligence: 9,
        dexterity: 12,
        wisdom: 8,
        constitution: 12,
        charisma: 6,
        // 5e Blindsight 30 — perceive without sight inside this radius.
        // Routes through the existing concealment-suppression chokepoint
        // that the Aboleth / Cloaker / Beholder Antimagic-Eye lanes also
        // consult. Same SpecialSense slot, smaller radius.
        senses: HashSet::from([SpecialSense::Blindsight(30)]),
        languages: HashSet::from([Language::Undercommon]),
        cr: 0.25,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
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

    fn make() -> ActorInstance {
        ActorInstance::from_creature_template(
            &GRIMLOCK_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn grimlock_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.25);
        assert_eq!(a.size(), Size::Medium);
        assert_eq!(a.creature_type(), CreatureType::Humanoid);
        assert!(a.find_action("spiked bone club").is_some());
    }

    #[test]
    fn grimlock_carries_blindsight() {
        // Pin the load-bearing sensory trait: Blindsight 30 ft is the
        // grimlock's defining mechanical edge over the same-CR Goblin
        // / Kobold tier. A future template-refactor that quietly
        // stripped the Blindsight would demote the grimlock to "an
        // orc with smaller dice", erasing the immune-to-Invisibility
        // / Darkness inside its perception radius half that defines
        // the cave-hunter identity.
        let a = make();
        assert!(a.senses().contains(&SpecialSense::Blindsight(30)));
    }
}
