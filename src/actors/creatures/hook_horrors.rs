use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{HOOK_HORROR_HOOK, HOOK_HORROR_MULTI};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Hook Horror — CR 3 large monstrosity. The underdark's iconic "vulture
/// crossed with a pair of giant pincer-hooks" predator. Slots above Owlbear
/// (CR 3) and below Hill Giant (CR 5) on the brute ladder; the load-bearing
/// per-round threat is a heavy two-hook melee burst at reach-2 (10 ft RAW).
///
/// Action lanes:
/// - **Hook Horror Multiattack** — 2 hook swings per Action. Vanilla
///   single-sub-attack Multiattack; each hook rolls its own d20 + STR vs
///   AC for the canonical CR-3 double-strike.
/// - **Hook Horror Hook** (standalone) — STR-based 1d10+STR piercing at
///   reach 2. The longer reach lets the hook horror threaten an extra
///   tile ring around its 2×2 Large footprint — flanking PCs eat
///   opportunity attacks at the longer envelope.
///
/// Damage envelope: no resistances or immunities — the hook horror is a
/// flesh-and-bone predator, not an elemental or fiend. Senses: Blindsight
/// 10 ft (RAW for picking up vibrations in the dark caves) plus
/// Darkvision 10 ft. No condition immunities.
///
/// Stat shape: AC 15 (chitinous carapace), ~75 average HP (10d10+20), STR
/// 18, DEX 10 — the slow-but-tough brute statline RAW uses. No legendary
/// resistances or actions. CR 3.
///
/// RAW also gives the hook horror "Keen Hearing" (advantage on Perception
/// vs sound) — omitted here since the engine doesn't model skill checks.
/// The load-bearing combat clause is the double-hook multi profile.
pub static HOOK_HORROR_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*HOOK_HORROR_MULTI);
    actions.push(&HOOK_HORROR_HOOK);
    CreatureTemplate {
        name: "Hook Horror",
        // 'H' is taken by Hippogriff / Hezrou; 'h' (lowercase) is free
        // in the monstrosity pool. Mnemonic for "hook horror".
        glyph: 'h',
        ac: 15,
        // 10d10+20 ≈ 75 average per MM (CR 3).
        hitpoints: "10d10+20".parse().unwrap(),
        speed: 30.,
        strength: 18,
        intelligence: 6,
        dexterity: 10,
        wisdom: 10,
        constitution: 15,
        charisma: 7,
        senses: HashSet::from([
            SpecialSense::Blindsight(10),
            SpecialSense::Darkvision(10),
        ]),
        cr: 3.0,
        size: Size::Large,
        creature_type: CreatureType::Monstrosity,
        actions,
        has_extra_attack: true,
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
    fn hook_horror_has_double_hook_multi() {
        let a = ActorInstance::from_creature_template(
            &HOOK_HORROR_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert!(a.find_action("hook horror multiattack").is_some());
        assert!(a.find_action("hook horror hook").is_some());
        assert_eq!(a.cr(), 3.0);
        assert_eq!(a.size(), Size::Large);
        assert!(a.has_extra_attack());
    }

    #[test]
    fn hook_horror_template_has_blindsight_and_darkvision() {
        // Template-only assertion since `ActorInstance` doesn't surface
        // a senses() accessor — the sense set propagates from the
        // template to the instance via `from_creature_template`.
        assert!(HOOK_HORROR_TEMPLATE.senses.contains(&SpecialSense::Blindsight(10)));
        assert!(HOOK_HORROR_TEMPLATE.senses.contains(&SpecialSense::Darkvision(10)));
    }
}
