use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::BAT_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Bat — CR 0 tiny beast. The canonical "fluttering nuisance" ambient
/// flier: a single bite swing for 1 piercing on a 1-HP frame, with the
/// signature **Blindsight 60** anti-stealth envelope inherited from the
/// echolocation cone. Sister to `GIANT_BAT_TEMPLATE` (CR ¼ large bat
/// with 1d6 bite) — same anti-stealth identity, lower CR, lower dice.
/// Slots beside the Hawk (CR 0 tiny diurnal flier), Rat (CR 0 tiny
/// rodent), Cat (CR 0 tiny climber) at the very bottom of the CR
/// ladder.
///
/// Action lane:
/// - **bat bite** — STR-based flat-1 piercing melee via the shared
///   `BAT_BITE` static. The bat's only swing. Single per-Action (no
///   multi); the bat dies to anything that connects, so the threat
///   profile is mobility + sensory anti-stealth, not damage.
///
/// **Echolocation** (RAW: the bat can't use its blindsight while
/// deafened) is partially modeled — Blindsight is a senses entry, not
/// a condition-gated trait, so a Deafened bat retains blindsight in
/// this engine. The simplification is small in practice: the Deafened
/// condition is rarely installed on a CR-0 bat. **Keen Hearing** (RAW:
/// advantage on Perception checks using hearing) is flavor-only — the
/// engine doesn't surface skill checks through combat.
///
/// Defensive identity: AC 12 (tiny + DEX), 1 HP. Vanilla beast envelope
/// — no resistances or condition immunities. The bat dies to any
/// solid hit; its load-bearing tactical value is "sees invisible /
/// hidden targets inside its 60-ft echolocation cone" rather than
/// damage output.
///
/// Stat shape: AC 12, ~1 HP (1d4-1 → floored at 1), STR 2, DEX 15,
/// CON 8, INT 2, WIS 12, CHA 4. Speed 30 — RAW: walking 5 ft + fly 30
/// ft. The engine collapses ground + fly to a single per-creature
/// speed; we pin to the fly speed since bats almost never walk. Senses:
/// Blindsight 60. Size Tiny. CR 0. XP: 10 per RAW.
pub static BAT_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&BAT_BITE);
    CreatureTemplate {
        name: "Bat",
        // 'b' (lowercase) — tiny flier silhouette. 'B' (uppercase) is
        // already taken by the Giant Bat / Brown Bear / Bugbear /
        // Bandit Captain cohort; lowercase 'b' reads as "tiny winged
        // nuisance" beside 'k' (Hawk), 'r' (Rat), and 'c' (Cat) at the
        // very bottom of the CR ladder. The team color disambiguates
        // from the same-glyph Owlbear / Berserker entries.
        glyph: 'b',
        ac: 12,
        // RAW: 1 (1d4 - 1) — engine floors HP rolls at 1.
        hitpoints: "1d4-1".parse().unwrap(),
        // Fly 30 — slower than the Hawk (fly 60) / Giant Bat (fly 60),
        // matching RAW's mundane-bat envelope (the bigger Giant Bat
        // mutates to fly 60). The engine collapses ground + fly to a
        // single per-creature speed.
        speed: 30.,
        strength: 2,
        intelligence: 2,
        dexterity: 15,
        wisdom: 12,
        constitution: 8,
        charisma: 4,
        // Blindsight 60 — the signature anti-stealth sense. Same
        // chokepoint as the Giant Bat / Cloaker / Beholder Antimagic
        // Eye consult: inside 60ft an invisible or hidden target loses
        // the attacker-disadvantage gate (`engine::attack`'s blindsight
        // chokepoint). The load-bearing tactical trait at the CR-0
        // tier — separates the bat from the Hawk / Owl cohort that
        // rely on plain Darkvision.
        senses: HashSet::from([SpecialSense::Blindsight(60)]),
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

    fn make() -> ActorInstance {
        ActorInstance::from_creature_template(
            &BAT_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn bat_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.0);
        assert_eq!(a.size(), Size::Tiny);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("bat bite").is_some());
    }

    #[test]
    fn bat_carries_blindsight() {
        // Pin the load-bearing sensory trait: Blindsight 60 is what
        // distinguishes the bat from the Hawk / Owl / Cat cohort on
        // the CR-0 tier. A future template-refactor that quietly
        // stripped the Blindsight would erase the echolocation niche
        // (the bat would become a slower, weaker hawk).
        let a = make();
        assert!(a.senses().contains(&SpecialSense::Blindsight(60)));
    }
}
