use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{SPRITE_LONGBOW, SPRITE_SHORTSWORD};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Sprite — CR ¼ tiny fey. The classic forest-scout cousin of the Pixie
/// (same CR ¼ slot) — sprites trade the Pixie's at-will area sleep dust
/// for a sleep-poisoned longbow that lands a single Asleep install on the
/// heaviest target at range. Slots at the bottom of the fey ladder
/// alongside Pixie (CR ¼) and below Dryad (CR 1) — the standoff variant
/// of the fey sleep-control archetype.
///
/// Action lanes:
/// - **sprite longbow** — DEX-based 1 piercing ranged at reach 8 (40 ft
///   RAW) with a CON 10 save rider for Asleep (10 rounds). The
///   load-bearing tactical clause: a sprite scout volley puts the heavy
///   melee threat to sleep before they close. Damage is incidental (a
///   flat 1 piercing); the rider does all the work.
/// - **sprite shortsword** — DEX-based 1 piercing melee. Fallback for
///   the rare case the sprite is pinned in melee; the AI should default
///   to the bow whenever there's clearance.
///
/// Defensive identity: the sprite has no resistances or immunities — the
/// 2-HP statline IS its design: glass-cannon scout that lands the sleep
/// opener and dies to any glancing blow. We omit Fey Ancestry / Magic
/// Resistance to keep the sprite distinct from the Pixie (the Pixie has
/// both; the sprite has neither in RAW). Heart Sight (the sprite's
/// alignment-reading clause) is a non-combat divination — no in-engine
/// consumer.
///
/// Stat shape: AC 15 (RAW armor-class — leather + DEX 18), 2 HP (1d4),
/// STR 3 (a stiff breeze knocks one over), DEX 18 (the high-DEX flying
/// fey signature), CON 10, INT 14, WIS 13, CHA 11. Speed 10 ground +
/// flight (RAW 40 ft fly) — the engine doesn't model 3D flight, so the
/// 10-ft ground speed reads as "tiny fey hovering and darting"; the
/// load-bearing slice is the 8-tile bow range, not the movement.
/// Languages: Common, Elvish, Sylvan. Size Tiny. CR ¼.
///
/// RAW also gives the sprite **Invisibility** (a passive at-will invis
/// from start of round to first action) — we omit the at-will invis
/// because (a) the engine's Invisibility chassis is concentration-gated
/// on the spell, and (b) the sleep-arrow opener is the load-bearing kit;
/// the invis is a sneak-attack-style flavor clause on top.
pub static SPRITE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    // Bow first so the AI picks it as the primary; the shortsword is the
    // pinned-in-melee fallback.
    actions.push(&*SPRITE_LONGBOW);
    actions.push(&SPRITE_SHORTSWORD);
    CreatureTemplate {
        name: "Sprite",
        // 's' (lowercase) — distinct from 'S' (Solar / Skeleton at
        // uppercase). 'p' is taken by Pixie; 's' for "sprite" reads as
        // the tiny fey scout's silhouette.
        glyph: 's',
        ac: 15,
        // 1d4 ≈ 2.5 average per MM (CR ¼). The 2-HP shape is the design:
        // a glass-cannon controller.
        hitpoints: "1d4".parse().unwrap(),
        // RAW speed line: Speed 10 ft., fly 40 ft.
        speed: 10.0,
        fly_speed: 40.0,
        strength: 3,
        intelligence: 14,
        dexterity: 18,
        wisdom: 13,
        constitution: 10,
        charisma: 11,
        senses: HashSet::new(),
        languages: HashSet::from([Language::Common, Language::Elvish, Language::Sylvan]),
        cr: 0.25,
        size: Size::Tiny,
        creature_type: CreatureType::Fey,
        actions,
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
    fn sprite_template_shape() {
        let a = ActorInstance::from_creature_template(
            &SPRITE_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 0.25);
        assert_eq!(a.size(), Size::Tiny);
        assert_eq!(a.creature_type(), CreatureType::Fey);
        // The sprite's two action lanes — bow (sleep-arrow) primary,
        // shortsword fallback in melee.
        assert!(a.find_action("sprite longbow").is_some());
        assert!(a.find_action("sprite shortsword").is_some());
    }

    #[test]
    fn sprite_has_no_magic_resistance_or_fey_ancestry() {
        let a = ActorInstance::from_creature_template(
            &SPRITE_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        // The sprite is the OTHER fey — Pixie has both Magic Resistance
        // and Fey Ancestry; the sprite has neither. Keeps the two CR-¼
        // fey distinct (Pixie: caster-flavored area sleep; Sprite:
        // archer-flavored single-target sleep arrow).
        assert!(!a.has_magic_resistance());
        // The 2-HP glass-cannon design — a sprite WILL die to anything
        // that lands, so the sleep arrow is its one tactical clause.
        assert!(!a.effectively_immune_to_condition(Condition::Charmed));
        assert!(!a.effectively_immune_to_condition(Condition::Asleep));
    }
}
