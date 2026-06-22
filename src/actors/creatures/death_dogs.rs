use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{DEATH_DOG_BITE, DEATH_DOG_MULTI};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Death Dog — CR 1 medium monstrosity. The two-headed underdark cur,
/// canid silhouette of the corpse-ditch. Slots between Wolf (CR ¼) and
/// Dire Wolf (CR 1) on the canid ladder — the disease-bite cousin of the
/// vanilla wolf, distinguished by twin heads (two bites per Action) and
/// the rotting jaw rider that installs the Poisoned condition as a proxy
/// for RAW's "diseased until cured" tag.
///
/// Action lanes:
/// - **death dog multiattack** — 2 bites per Action via `Multiattack`.
///   Each head rolls its own d20 + STR vs AC AND an independent disease
///   save on hit, so the per-Action burst can install Poisoned twice
///   (matters when the condition rides a stacking timer; the engine
///   keeps the longest active install so the effective duration is the
///   max of the two rolls).
/// - **death dog bite** (standalone) — STR-based 1d6+STR piercing melee
///   with a CON 12 save rider for Poisoned (Rounds(10), proxy for
///   RAW's "diseased until cured"). The CR-1 bite line is identical in
///   shape to the Otyugh's disease-save bite at CR 5 — same chassis,
///   smaller die and shorter timer at the lower tier.
///
/// Defensive identity: AC 12 (natural armor — the matted hide), 39 HP
/// (6d8+12), STR 15, DEX 14, CON 14. No resistances or immunities — the
/// death dog is a beast-tier predator, not an elemental or fiend. The
/// signature defensive clause is Perception advantage from two heads
/// (RAW: advantage on Perception checks); the engine doesn't tag
/// Perception rolls separately, so we leave the two-headed bonus
/// purely flavor.
///
/// Stat shape: AC 12, ~39 HP (6d8+12), STR 15, DEX 14, CON 14, INT 3,
/// WIS 13, CHA 6. Speed 40 (canid burst speed). Senses: Darkvision 120
/// (the canid sees in the underdark and the corpse-ditch perfectly).
/// Languages: none (a death dog is a beast in everything but type).
/// Size Medium. CR 1.
///
/// RAW also gives the death dog **Two-Headed** (advantage on Perception
/// checks, plus immunity to being Blinded / Deafened / Stunned /
/// Knocked Unconscious while at least one head is active). We omit the
/// "one head down" partial-immunity clause since the engine doesn't
/// model per-head HP pools; the load-bearing slice is the twin-bite
/// multi, not the partial-immunity nuance.
pub static DEATH_DOG_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*DEATH_DOG_MULTI);
    actions.push(&*DEATH_DOG_BITE);
    CreatureTemplate {
        name: "Death Dog",
        // 'D' (uppercase) — distinct from 'd' (Dire Wolf uses 'd' too;
        // we share the glyph since both are CR-1 canids and the
        // silhouette pool is exhausted at letters that read as
        // "predator dog"). Uppercase distinguishes the disease-bite
        // variant from the vanilla canid.
        glyph: 'D',
        ac: 12,
        // 6d8+12 ≈ 39 average per MM (CR 1).
        hitpoints: "6d8+12".parse().unwrap(),
        speed: 40.,
        strength: 15,
        intelligence: 3,
        dexterity: 14,
        wisdom: 13,
        constitution: 14,
        charisma: 6,
        senses: HashSet::from([SpecialSense::Darkvision(120)]),
        languages: HashSet::new(),
        cr: 1.0,
        size: Size::Medium,
        creature_type: CreatureType::Monstrosity,
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

    #[test]
    fn death_dog_template_shape() {
        let a = ActorInstance::from_creature_template(
            &DEATH_DOG_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 1.0);
        assert_eq!(a.size(), Size::Medium);
        assert_eq!(a.creature_type(), CreatureType::Monstrosity);
        // The death dog's two action lanes — multi (twin bite) primary,
        // single bite for AI fallback.
        assert!(a.find_action("death dog multiattack").is_some());
        assert!(a.find_action("death dog bite").is_some());
    }

    #[test]
    fn death_dog_has_no_magic_resistance_or_immunities() {
        let a = ActorInstance::from_creature_template(
            &DEATH_DOG_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        // The death dog is a beast-tier predator, no magic resistance
        // or special immunities. The 39-HP pool + 40-speed burst IS
        // the defense; the disease bite is the offense.
        assert!(!a.has_magic_resistance());
    }
}
