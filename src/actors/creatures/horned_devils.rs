use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    HORNED_DEVIL_FORK, HORNED_DEVIL_HURLED_FLAME, HORNED_DEVIL_MULTI, HORNED_DEVIL_TAIL,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{
    CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense,
};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Horned Devil (Malebranche) — CR 11 large fiend (lawful-evil devil).
/// The polearm-wielding lieutenant of the Nine Hells: a hulking horned
/// brute with infernal trident and barbed tail. Slots between Bone Devil
/// (CR 9) and Erinyes (CR 12) on the devil ladder, and below the
/// boss-tier Pit Fiend (CR 20) / Balor (CR 19) on the fiend mid-boss
/// bench. The canonical "devil with reach" combatant — projects threat
/// through a tile of empty space via its 10ft fork + tail and lobs
/// hellfire from across the battlefield.
///
/// Action lanes:
/// - **horned devil multiattack** — 2 forks + 1 tail per Action via the
///   shared `CompoundAttack` chassis. Heterogeneous compound (piercing
///   forks + piercing tail) — the tail carries the Infernal Wound rider
///   (CON-17 save vs Poisoned 10 rounds, proxy for RAW's no-regen +
///   ongoing damage clause). ~25 average damage per Action against a
///   single target, plus the wound rider.
/// - **horned devil fork** (standalone) — STR-based 2d8+STR piercing
///   melee at reach 10ft (gap 2). The primary damage lane; the longer
///   reach gives the devil a real threat envelope around its 2×2 Large
///   footprint.
/// - **horned devil tail** (standalone) — STR-based 1d8+STR piercing
///   melee at reach 10ft with the Infernal Wound save-or-Poisoned rider.
/// - **hurled flame** — ranged CHA-attack fire bolt at 150ft range (we
///   cap at 30 tiles). 4d6 fire on hit. At-will (no recharge); the
///   devil's "stay out of melee and lob hellfire" stand-off lane.
///
/// Defensive identity: AC 18 (natural armor — the infernal hide), 178
/// HP (17d10+85). The standard mid-tier devil envelope: resistant to
/// cold (devil family) + non-magical BPS (RAW "from nonmagical attacks";
/// we collapse to flat resistance since the engine doesn't tag magical/
/// mundane weapons). Immune to fire + poison (the canonical hellish
/// damage envelope). Immune to Poisoned (the devil's mind-and-body
/// envelope).
///
/// Magic Resistance is **on** — RAW: advantage on saves vs spells and
/// other magical effects. The standard caster-counter lane shared by
/// the mid-and-upper devils.
///
/// Stat shape: AC 18, ~178 HP (17d10+85), STR 22, DEX 17, CON 21,
/// INT 12, WIS 16, CHA 17. Speed 20 (the horned devil's bulk hurts its
/// land speed; in RAW it has a fly speed of 60 which we don't yet
/// model). Senses: Darkvision 120. Languages: Infernal (the devil
/// tongue). Telepathy 120ft RAW — we drop telepathy since the engine
/// doesn't model it as a language. Size Large. CR 11.
pub static HORNED_DEVIL_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*HORNED_DEVIL_MULTI);
    actions.push(&HORNED_DEVIL_FORK);
    actions.push(&HORNED_DEVIL_TAIL);
    actions.push(&*HORNED_DEVIL_HURLED_FLAME);
    CreatureTemplate {
        name: "Horned Devil",
        // 'H' (uppercase) — distinct from existing 'h' (Hobgoblin /
        // Hyena) and 'H' (Hezrou — different uppercase but only one in
        // the demon pool). 'H' for the horned silhouette.
        glyph: 'H',
        ac: 18,
        // 17d10+85 ≈ 178 average per MM (CR 11).
        hitpoints: "17d10+85".parse().unwrap(),
        speed: 20.,
        strength: 22,
        intelligence: 12,
        dexterity: 17,
        wisdom: 16,
        constitution: 21,
        charisma: 17,
        senses: HashSet::from([SpecialSense::Darkvision(120)]),
        languages: HashSet::from([Language::Infernal]),
        cr: 11.0,
        size: Size::Large,
        creature_type: CreatureType::Fiend,
        actions,
        // Mid-tier devil damage envelope: cold + non-magical BPS
        // resistance, fire + poison immunity. The cold resistance is the
        // shared devil-family trait (the Nine Hells are cold below the
        // top layers); the fire / poison immunity is the canonical
        // hellish envelope.
        damage_modifiers: HashMap::from([
            (DamageType::Cold, DamageModifier::Resistance),
            (DamageType::Bludgeoning, DamageModifier::Resistance),
            (DamageType::Piercing, DamageModifier::Resistance),
            (DamageType::Slashing, DamageModifier::Resistance),
            (DamageType::Fire, DamageModifier::Immunity),
            (DamageType::Poison, DamageModifier::Immunity),
        ]),
        condition_immunities: HashSet::from([Condition::Poisoned]),
        // Magic Resistance: advantage on saves vs spells / magical
        // effects. Standard mid-tier devil trait.
        has_magic_resistance: true,
        has_extra_attack: true,
        // 5e **Devil's Sight** — "magical darkness doesn't impede this
        // devil's darkvision." Carried by every devil in the bestiary,
        // and the one thing in the game that sees through the Darkness
        // spell. Before the lighting layer existed the trait was
        // approximated as a generous darkvision radius, which was the
        // closest the engine could get to it and got the crucial half
        // exactly backwards: RAW darkvision is precisely what magical
        // darkness defeats.
        features: HashSet::from([
            crate::actions::class_features::DEVILS_SIGHT_TAG,
            // 5e **Magic Weapons**: "the devil's weapon attacks
            // are magical."
            crate::actions::class_features::MAGICAL_ATTACKS_TAG,
        ]),
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
    fn horned_devil_template_shape() {
        let a = ActorInstance::from_creature_template(
            &HORNED_DEVIL_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 11.0);
        assert_eq!(a.size(), Size::Large);
        assert_eq!(a.creature_type(), CreatureType::Fiend);
        // The horned devil's four action lanes — multi (forks + tail)
        // primary, fork / tail standalone for AI fallback, plus the
        // ranged hurled flame for stand-off.
        assert!(a.find_action("horned devil multiattack").is_some());
        assert!(a.find_action("horned devil fork").is_some());
        assert!(a.find_action("horned devil tail").is_some());
        assert!(a.find_action("hurled flame").is_some());
    }

    #[test]
    fn horned_devil_has_devil_envelope() {
        let a = ActorInstance::from_creature_template(
            &HORNED_DEVIL_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        // Mid-tier devil envelope: fire / poison immune, cold + BPS
        // resistant, magic resistance, poisoned-condition immunity.
        assert_eq!(
            a.damage_modifier(DamageType::Fire),
            Some(DamageModifier::Immunity)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Poison),
            Some(DamageModifier::Immunity)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Cold),
            Some(DamageModifier::Resistance)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Bludgeoning),
            Some(DamageModifier::Resistance)
        );
        assert!(a.has_magic_resistance());
        assert!(a.effectively_immune_to_condition(Condition::Poisoned));
    }
}
