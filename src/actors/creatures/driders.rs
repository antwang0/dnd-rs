use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    DRIDER_BITE, DRIDER_LONGBOW, DRIDER_LONGSWORD, DRIDER_MULTI,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Drider — CR 6 large monstrosity. Corrupted drow elevated to a spider-
/// hybrid form by Lolth's curse. Slots between Cambion (CR 5) and Vrock
/// (CR 6) on the mid-CR ladder — the "drow + spider" lane to the
/// fiend / demon options at the same tier.
///
/// Action lanes:
/// - **drider multiattack** — 2 longsword swings + 1 bite per Action via
///   `CompoundAttack`. Heterogeneous limbs combine cleanly so each limb
///   keeps its own dice tier (1d8 sword + 1d4 bite).
/// - **drider longsword** (standalone) — 1d8+STR slashing melee. Exposed
///   so the AI can fall back to a single swing when bonus-action-tagged
///   or moving in.
/// - **drider longbow** (standalone) — 1d8+DEX piercing ranged. The
///   hit-and-run alternative when an enemy is out of melee reach;
///   range 12 tiles (≈60ft normal, 100ft max — under the RAW long-
///   range threshold for indoor maps).
/// - **drider bite** (standalone) — 1d4+STR piercing + 4d8 poison rider
///   (CON 13 save for half). The signature spider-half lethality —
///   even on a save the target eats 2d8 poison damage.
///
/// Defensive identity: AC 19 (natural armor — Lolth's curse hardens the
/// chitinous form). The drider gets **Spider Climb** as a passive
/// (installed via the condition lane at instantiation) for the +30ft
/// climbing-speed bump, mirroring how Slippers of Spider Climbing
/// install the same condition on pickup. No magic resistance — the
/// drider is a corrupted mortal, not a fey / outsider.
///
/// Stat shape: AC 19, ~123 HP (13d10+52), STR 16, DEX 19, CON 18, INT 13,
/// WIS 16, CHA 12. Speed 30. Senses: Darkvision 120 (drow-tier dark
/// vision). Languages: Elvish, Undercommon. Size Large. CR 6.
///
/// RAW also gives the drider:
/// - **Fey Ancestry** (advantage on saves vs Charmed, sleep immunity) —
///   we model via `has_fey_ancestry` flag (covered through the
///   `dynamic_immunity_to` chokepoint).
/// - **Sunlight Sensitivity** (disadvantage on attack rolls in direct
///   sunlight) — the engine doesn't model day/night cycles, so this
///   trait has no in-engine consumer. Omitted.
/// - **Innate Spellcasting** (Dancing Lights, Darkness, Faerie Fire,
///   Levitate) — omitted per the same convention as Cambion /
///   Couatl: innate caster picks don't surface through the engine's
///   action chassis.
/// - **Web Walker** (immune to web movement restraint) — omitted (no
///   in-engine consumer; the engine's Web spell uses the standard
///   Restrained condition lane and the drider would resist via its
///   raw CR-tier saves).
pub static DRIDER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*DRIDER_MULTI);
    actions.push(&DRIDER_LONGSWORD);
    actions.push(&DRIDER_LONGBOW);
    actions.push(&*DRIDER_BITE);
    CreatureTemplate {
        name: "Drider",
        // 'D' (uppercase) — distinct from 'd' (Dryad / Dragonborn use 'd').
        // Large four-limbed silhouette warrants the uppercase glyph.
        glyph: 'D',
        ac: 19,
        // 13d10+52 ≈ 123 average per MM (CR 6).
        hitpoints: "13d10+52".parse().unwrap(),
        speed: 30.,
        strength: 16,
        intelligence: 13,
        dexterity: 19,
        wisdom: 16,
        constitution: 18,
        charisma: 12,
        senses: HashSet::from([SpecialSense::Darkvision(120)]),
        languages: HashSet::from([Language::Elvish, Language::Undercommon]),
        cr: 6.0,
        size: Size::Large,
        creature_type: CreatureType::Monstrosity,
        actions,
        // Fey Ancestry — drow heritage carries through to the drider form.
        // Approximated as full immunity to Charmed (+ magical Sleep) at the
        // `dynamic_immunity_to` chokepoint.
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
    fn drider_template_shape() {
        let a = ActorInstance::from_creature_template(
            &DRIDER_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 6.0);
        assert_eq!(a.size(), Size::Large);
        assert!(a.find_action("drider multiattack").is_some());
        assert!(a.find_action("drider longsword").is_some());
        assert!(a.find_action("drider longbow").is_some());
        assert!(a.find_action("drider bite").is_some());
    }

    #[test]
    fn drider_has_fey_ancestry_immunities() {
        let a = ActorInstance::from_creature_template(
            &DRIDER_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        // Fey Ancestry — drow heritage. Charm + Asleep immunity via the
        // `dynamic_immunity_to` chokepoint.
        assert!(a.effectively_immune_to_condition(Condition::Charmed));
        assert!(a.effectively_immune_to_condition(Condition::Asleep));
    }
}
