use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{GHAST_BITE, GHAST_CLAWS, GHAST_MULTI};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Ghast — CR 2 undead. The "upgraded ghoul" tier: heavier dice, a
/// proper bite + claws multiattack, and the same DC-10 paralysis
/// auto-crit envelope on a longer 1-minute (10-round) timer. Slots
/// directly above the Ghoul (CR 1) and below the Wight (CR 3) on the
/// undead ladder — a bigger, meaner version of the same paralyze-then-
/// crit lock-down loop. Two ghasts working together can perma-lock a
/// PC on a single failed save: paralysis turns every melee hit within
/// 5 ft into an auto-crit, and the partner is already adjacent to
/// cash in.
///
/// Action lanes:
/// - **ghast bite** — STR-based 2d8+STR piercing melee via the shared
///   `GHAST_BITE` static. Heavier-die secondary swing — pure damage,
///   no rider. RAW: 2d8+3 ≈ 12.
/// - **ghast claws** — STR-based 2d6+STR slashing melee with a DC 10
///   CON save-or-Paralyzed-for-10-rounds rider on a confirmed hit.
///   Routes through the shared `WeaponWithSaveCondition` chassis
///   alongside Wolf Bite / Dire Wolf Bite. The marquee threat.
/// - **ghast multiattack** — 1 bite + 1 claws per Action via the
///   shared `CompoundAttack` chassis (`GHAST_MULTI`). Two-hit Action
///   lands ~12 piercing + ~10 slashing + a paralysis save against a
///   medium-AC target.
///
/// Defensive identity: AC 13 (DEX-based), 36 HP (8d8). Standard undead
/// damage envelope — poison immunity. Standard undead condition
/// envelope — immune to Charmed (the ghast can't be talked down) AND
/// Poisoned (a corpse can't be sickened further). Distinct from the
/// ghoul in two ways: ghast HP is roughly double and the paralysis
/// timer is roughly 5× longer (10 rounds vs 2).
///
/// Stat shape: AC 13, ~36 HP (8d8), STR 16, DEX 17, CON 10, INT 11,
/// WIS 10, CHA 8. Speed 30. Senses: Darkvision 60. Languages: Common.
/// Size Medium. CR 2. XP: 450 per RAW.
///
/// **Stench** (RAW): every creature within 5 ft starts of its turn
/// makes a DC 10 CON save or is Poisoned until the start of its next
/// turn. Omitted here as a deliberate scope cut — the engine doesn't
/// surface start-of-turn aura saves through a generic chassis yet,
/// and the load-bearing combat clauses (claws-paralysis + bite + multi)
/// already define the ghast's per-round footprint. **Turning Defiance**
/// (RAW: advantage on saves vs being turned, applies to the ghast and
/// undead within 30 ft) is similarly flavor-only — the engine's Turn
/// Undead lane runs through a single save, not the radius aura.
pub static GHAST_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GHAST_BITE);
    actions.push(&GHAST_CLAWS);
    actions.push(&*GHAST_MULTI);
    CreatureTemplate {
        name: "Ghast",
        // 'U' — shared with Ghoul / Bullette: the ghast is the bigger
        // sibling of the ghoul, so the shared 'U' (Undead) silhouette
        // reads correctly on the map. Team color disambiguates if both
        // appear in the same encounter.
        glyph: 'U',
        ac: 13,
        // 8d8 = 36 average per MM (CR 2).
        hitpoints: "8d8".parse().unwrap(),
        speed: 30.,
        strength: 16,
        intelligence: 11,
        dexterity: 17,
        wisdom: 10,
        constitution: 10,
        charisma: 8,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common]),
        cr: 2.0,
        size: Size::Medium,
        creature_type: CreatureType::Undead,
        actions,
        // Standard undead damage envelope: poison immunity.
        damage_modifiers: HashMap::from([(DamageType::Poison, DamageModifier::Immunity)]),
        // Standard undead condition envelope: Charmed / Poisoned
        // immunity. Distinct from the ghoul (which doesn't carry
        // Poisoned immunity in some MM printings — we standardize on
        // both for the undead family here).
        condition_immunities: HashSet::from([
            // SRD 5.2 "Immunities Poison; Charmed, Exhaustion, Poisoned".
            Condition::Charmed,
            Condition::Exhausted,
            Condition::Poisoned,
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

    fn make() -> ActorInstance {
        ActorInstance::from_creature_template(
            &GHAST_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn ghast_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 2.0);
        assert_eq!(a.size(), Size::Medium);
        assert_eq!(a.creature_type(), CreatureType::Undead);
        assert!(a.find_action("ghast bite").is_some());
        assert!(a.find_action("ghast claws").is_some());
        assert!(a.find_action("ghast multiattack").is_some());
    }

    #[test]
    fn ghast_has_undead_envelope() {
        // Pin the load-bearing defensive clauses: poison immunity
        // (corpse can't be poisoned further) + charm / poisoned condition
        // immunity (standard undead lane). A future refactor of the
        // undead family shouldn't strip these.
        let a = make();
        assert_eq!(
            a.damage_modifier(DamageType::Poison),
            Some(DamageModifier::Immunity)
        );
        assert!(a.effectively_immune_to_condition(Condition::Charmed));
        assert!(a.effectively_immune_to_condition(Condition::Poisoned));
    }
}
