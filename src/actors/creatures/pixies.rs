use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::PIXIE_SLEEP_DUST;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Pixie — CR ¼ tiny fey. The classic 1-HP gossamer-winged trickster of the
/// woodlands. Slots at the bottom of the fey CR ladder (below Dryad at
/// CR 1 / Green Hag at CR 3 / Sea Hag-flavored mid-tier) — the cheapest fey
/// pick in the pool. The load-bearing combat clause is the area-effect
/// `PIXIE_SLEEP_DUST` Action: a 5 ft burst at 30 ft range, DC 12 WIS or
/// Asleep for 10 rounds. RAW gives the pixie a wide innate-spellcasting kit
/// (Confusion / Polymorph / Sleep / Dispel Magic / etc.); we surface the
/// load-bearing tactical clause (Sleep) and leave the broader kit for the
/// engine's general spell pool a future pixie chassis could pull from.
///
/// Defensive identity: Magic Resistance (advantage on saves vs spells)
/// plus Fey Ancestry (Charm + Sleep immunity at the
/// `dynamic_immunity_to` chokepoint) — the standard fey defense bundle
/// shared with Dryad. Combined with the 1 HP statline this makes the
/// pixie a glass-cannon controller: it WILL die to the first attack
/// that lands, but it lands its sleep dust at start-of-fight tempo
/// before anyone closes.
///
/// Stat shape: AC 15 (the gossamer "hard to swat" lane), 1 HP (1d4-1),
/// STR 2 (a child could break one), DEX 20 (signature pixie speed),
/// INT 10, WIS 14, CHA 15 (spellcasting ability). Senses: none beyond
/// the implied "fey perception" — RAW gives only passive Perception 14
/// (skill-tracked) so we leave the senses set empty. Languages: Sylvan.
/// Size Tiny — fits in a single tile but reads as a smaller target for
/// flavor. CR ¼.
///
/// RAW also gives the pixie Superior Invisibility (a permanent Greater
/// Invisibility-style effect). The engine has the `Invisible` condition
/// and Greater Invisibility lane; the load-bearing slice is the at-will
/// invisibility, which we surface via the `Invisible` condition pre-
/// installed at instantiation — covered through the standard Invisibility
/// spell + concentration drop mechanics elsewhere, but the pixie's
/// always-on invisibility doesn't fit the concentration model cleanly,
/// so we omit it here and lean on the sleep-dust + 1-HP design instead.
/// Future "always invisible" template flag could revisit.
pub static PIXIE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*PIXIE_SLEEP_DUST);
    CreatureTemplate {
        name: "Pixie",
        // 'p' (lowercase) — distinct from 'P' (Paladin / Pit Fiend uses
        // separate glyph), reads as a tiny fey.
        glyph: 'p',
        ac: 15,
        // 1d4-1 averages 1.5; we keep the rolled value (max(0)) so a
        // lucky roll can give 2-3 HP but the pixie still dies to any
        // glancing blow.
        hitpoints: "1d4-1".parse().unwrap(),
        // RAW speed line: Speed 10 ft., fly 30 ft.
        speed: 10.0,
        fly_speed: 30.0,
        strength: 2,
        intelligence: 10,
        dexterity: 20,
        wisdom: 14,
        constitution: 8,
        charisma: 15,
        senses: HashSet::new(),
        languages: HashSet::from([Language::Sylvan]),
        cr: 0.25,
        size: Size::Tiny,
        creature_type: CreatureType::Fey,
        actions,
        // 5e Magic Resistance — advantage on saves vs spells / magical
        // effects. Standard fey defense lane (shared with Dryad).
        has_magic_resistance: true,
        // Fey Ancestry — advantage on saves vs Charmed, and magic can't
        // put them to sleep. Approximated as full Charmed + Asleep
        // immunity at the `dynamic_immunity_to` chokepoint.
        has_fey_ancestry: true,
        ..CreatureTemplate::defaults()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::conditions::Condition;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::Coordinate;

    #[test]
    fn pixie_template_shape() {
        let a = ActorInstance::from_creature_template(
            &PIXIE_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 0.25);
        assert_eq!(a.size(), Size::Tiny);
        assert!(a.find_action("sleep dust").is_some());
    }

    #[test]
    fn pixie_has_fey_ancestry_immunities() {
        let a = ActorInstance::from_creature_template(
            &PIXIE_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        // Fey ancestry approximates as Charmed / Asleep immunity. A pixie
        // cannot be put to sleep by another pixie's own sleep dust.
        assert!(a.effectively_immune_to_condition(Condition::Charmed));
        assert!(a.effectively_immune_to_condition(Condition::Asleep));
        assert!(a.has_magic_resistance());
    }
}
