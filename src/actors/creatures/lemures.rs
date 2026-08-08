use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::LEMURE_FIST;
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Lemure — CR 0 medium fiend (lawful-evil devil, the lowest tier).
/// A formless mass of viscous flesh — the petitioner-soul stage of every
/// devil. Slots at the bottom of the fiend ladder below Dretch (CR ¼)
/// and Imp / Quasit (CR 1) — the cheapest fiend pick in the bestiary,
/// designed as a swarm-grunt that dies in one good hit but tanks small
/// damage via the devil envelope of resistances.
///
/// Action lanes:
/// - **lemure fist** (single Action) — STR-based 1d4 bludgeoning melee,
///   no STR mod. A flat 2 (1d4) bludgeoning slap. The lemure has no
///   other actions — it's a single-attack grunt by RAW.
///
/// Defensive identity: resistant to cold (devils universally), immune
/// to fire (the canonical hellish damage envelope) and poison. Immune
/// to Charmed / Frightened / Poisoned conditions (the devil's
/// mind-affecting resistance bundle). The "non-magical weapons" RAW
/// resistance clause is omitted since the engine doesn't tag attacks
/// magical/mundane.
///
/// RAW: Devil's Sight (sees normally in magical darkness) — modeled as
/// generous Darkvision 120 since the engine doesn't yet distinguish
/// magical vs mundane darkness. Hellish Rejuvenation (a lemure that
/// dies in Avernus reforms after 1d10 days) is a flavor clause with no
/// in-combat effect, dropped from the template.
///
/// Stat shape: AC 7 (the wretched, formless body — the lowest AC in
/// the bestiary), ~13 HP (2d8+4), STR 10, DEX 5, CON 11, INT 1, WIS 11,
/// CHA 3. Speed 15 (the petitioner shuffle). Senses: Darkvision 120.
/// Languages: understanding of Infernal but cannot speak (we drop the
/// listener-only flag and use an empty language set since the engine
/// doesn't track comprehension-only languages). Size Medium. CR 0.
pub static LEMURE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&LEMURE_FIST);
    CreatureTemplate {
        name: "Lemure",
        // 'l' (lowercase) — distinct from 'L' (Lich uppercase). The
        // lowercase reads as a low-tier grunt; pairs with 'd' (Dretch)
        // and 'q' (Quasit) in the lowercase-fiend cohort.
        glyph: 'l',
        ac: 7,
        // 2d8+4 ≈ 13 average per MM (CR 0).
        hitpoints: "2d8+4".parse().unwrap(),
        speed: 15.,
        strength: 10,
        intelligence: 1,
        dexterity: 5,
        wisdom: 11,
        constitution: 11,
        charisma: 3,
        senses: HashSet::from([SpecialSense::Darkvision(120)]),
        // RAW: Lemure "understands Infernal but can't speak". We drop
        // the listener-only flag since the engine doesn't surface
        // comprehension-only languages. Other devils (Bearded, Pit
        // Fiend) speak Infernal directly; this entry stays empty.
        languages: HashSet::new(),
        cr: 0.0,
        size: Size::Medium,
        creature_type: CreatureType::Fiend,
        actions,
        damage_modifiers: HashMap::from([
            (DamageType::Cold, DamageModifier::Resistance),
            (DamageType::Fire, DamageModifier::Immunity),
            (DamageType::Poison, DamageModifier::Immunity),
        ]),
        // Devil mind-affecting immunity bundle. Charmed / Frightened
        // are the load-bearing clauses; Poisoned is also condition-
        // listed in RAW but the Poison damage immunity already covers
        // most install paths — we list it explicitly so a non-damage
        // Poisoned source (e.g. environmental fumes) bounces too.
        condition_immunities: HashSet::from([
            Condition::Charmed,
            Condition::Frightened,
            Condition::Poisoned,
        ]),
        // 5e **Devil's Sight** — "magical darkness doesn't impede this
        // devil's darkvision." Carried by every devil in the bestiary,
        // and the one thing in the game that sees through the Darkness
        // spell. Before the lighting layer existed the trait was
        // approximated as a generous darkvision radius, which was the
        // closest the engine could get to it and got the crucial half
        // exactly backwards: RAW darkvision is precisely what magical
        // darkness defeats.
        features: HashSet::from([crate::actions::class_features::DEVILS_SIGHT_TAG]),
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
    fn lemure_template_shape() {
        let a = ActorInstance::from_creature_template(
            &LEMURE_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 0.0);
        assert_eq!(a.size(), Size::Medium);
        assert_eq!(a.creature_type(), CreatureType::Fiend);
        // Single attack lane — the lemure has only one action.
        assert!(a.find_action("lemure fist").is_some());
    }

    #[test]
    fn lemure_has_devil_damage_envelope() {
        let a = ActorInstance::from_creature_template(
            &LEMURE_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        // Devil damage envelope: fire / poison immune, cold resistant.
        // No magic resistance — the lemure is the cheapest devil and
        // RAW lacks the trait.
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
        assert!(!a.has_magic_resistance());
    }

    #[test]
    fn lemure_is_mind_affecting_immune() {
        let a = ActorInstance::from_creature_template(
            &LEMURE_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        // Devils shrug off mind-affecting conditions. The lemure's INT
        // 1 / CHA 3 reads as a husk — Charmed, Frightened, Poisoned
        // all bounce off the install chokepoint.
        assert!(a.effectively_immune_to_condition(Condition::Charmed));
        assert!(a.effectively_immune_to_condition(Condition::Frightened));
        assert!(a.effectively_immune_to_condition(Condition::Poisoned));
    }
}
