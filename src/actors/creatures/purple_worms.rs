use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    PURPLE_WORM_BITE, PURPLE_WORM_MULTI, PURPLE_WORM_TAIL_STINGER,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};


use std::collections::HashSet;
use std::sync::LazyLock;

/// Purple Worm — CR 15 Gargantuan monstrosity. The iconic dungeon
/// devourer: a tunneling colossus the size of a freight train with a
/// blunt biting maw at one end and a venomous segmented tail at the
/// other. The signature CR-15 brute on the non-dragon, non-undead
/// ladder — sits between the Aboleth (CR 10) and the Adult Red Dragon
/// (CR 17) on the boss-tier shelf with no boss-form spellcaster
/// envelope (it doesn't have Magic Resistance, doesn't cast — it just
/// crushes and poisons).
///
/// Action lanes:
/// - **purple worm bite** — STR-based 3d8+STR piercing melee at
///   reach 2 (10 ft). The bulk-damage limb; averages ~17 per swing on
///   the gargantuan frame. Vanilla `SimpleWeapon::reach_melee`.
/// - **purple worm tail stinger** — STR-based 3d6+STR piercing melee
///   at reach 2 with a DC 19 CON save-or-7d6-poison rider. Routes
///   through the shared `save_or_damage_rider` chassis (same lane as
///   Imp Sting, Spider Bite, Wyvern Stinger), so per-target poison
///   resistance / immunity collapses cleanly. The save DC is CR-15
///   boss-tier — most non-poison-immune mid-tier PCs eat the rider on
///   anything but a clean save.
/// - **purple worm multiattack** — 1 bite + 1 stinger per Action via
///   the shared `CompoundAttack` chassis. Mixed-limb mandatory pairing
///   matching SRD RAW — the AI can't pick "two bites" by spamming the
///   bite alone.
///
/// Defensive identity: AC 18 (natural armor — the thick chitinous
/// segments of an underground giant), 247 HP (15d20+90 ≈ 247). No
/// damage resistance / immunity — RAW the purple worm is a pure brute,
/// not a magical / elemental holdover. Tunneler trait (RAW: "the worm
/// can burrow through solid rock at half its burrow speed") lives as
/// flavor on the template only — the engine isn't 3D and collapses
/// burrow movement onto the surface speed. Swallow (RAW: a separate
/// action that one-shots Medium-or-smaller targets on a failed save)
/// is omitted as a deliberate gameplay scope cut — the engine doesn't
/// model "swallowed" as a containment state distinct from Grappled,
/// and replicating the RAW "you take acid damage every turn until you
/// cut your way out" loop without that state would distort the worm's
/// damage budget. The Tail Stinger's save-or-7d6-poison clause already
/// carries the worm's burst-damage identity at this tier.
///
/// Stat shape: AC 18, ~247 HP (15d20+90), STR 28, DEX 7, CON 22,
/// INT 1, WIS 8, CHA 4. Speed 50 (RAW 50 ft + 30 ft burrow — we collapse
/// to the faster of the two since the engine isn't 3D). Senses:
/// Blindsight 30, Tremorsense 60 (the worm "sees" through ground
/// vibrations). Languages: none (CR-15 dumb brute). Size Gargantuan.
/// CR 15. XP: 13,000 per RAW.
pub static PURPLE_WORM_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&PURPLE_WORM_BITE);
    actions.push(&PURPLE_WORM_TAIL_STINGER);
    actions.push(&*PURPLE_WORM_MULTI);
    CreatureTemplate {
        name: "Purple Worm",
        // 'P' (uppercase) — distinct mnemonic for Purple Worm. The
        // capital reads as a tunneling-segmented silhouette. 'P' is
        // currently used by Pit Fiend (in different encounter pools)
        // but the silhouette / size category disambiguation is clear:
        // Pit Fiend is a Large bipedal devil, Purple Worm is a
        // Gargantuan tunneling serpent. If glyph collisions become
        // a problem at any join site, swap to 'W' (used by Wyvern /
        // Werewolf) or '#' (the snake-segment fallback).
        glyph: 'P',
        ac: 18,
        // 15d20+90 ≈ 247 average per MM (CR 15).
        hitpoints: "15d20+90".parse().unwrap(),
        speed: 50.,
        strength: 28,
        intelligence: 1,
        dexterity: 7,
        wisdom: 8,
        constitution: 22,
        charisma: 4,
        senses: HashSet::from([
            SpecialSense::Blindsight(30),
            SpecialSense::Tremorsense(60),
        ]),
        languages: HashSet::new(),
        cr: 15.0,
        size: Size::Gargantuan,
        creature_type: CreatureType::Monstrosity,
        actions,
        // CR-15 RAW saves: STR + CON proficient. The worm's body shrugs
        // off physical effects and toxin counter-effects; its DEX / INT
        // / WIS / CHA saves remain raw ability rolls.
        proficient_saves: HashSet::from([
            crate::engine::types::AbilityScoreType::Strength,
            crate::engine::types::AbilityScoreType::Constitution,
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
    fn purple_worm_template_shape() {
        let a = ActorInstance::from_creature_template(
            &PURPLE_WORM_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 15.0);
        assert_eq!(a.size(), Size::Gargantuan);
        assert_eq!(a.creature_type(), CreatureType::Monstrosity);
        // Three action lanes for the bite / stinger / multi triplet.
        assert!(a.find_action("purple worm bite").is_some());
        assert!(a.find_action("purple worm tail stinger").is_some());
        assert!(a.find_action("purple worm multiattack").is_some());
    }

    #[test]
    fn purple_worm_multiattack_executes_one_bite_and_one_stinger() {
        // Pin the load-bearing combat clause: the worm's per-Action shape
        // is ONE bite + ONE stinger, not two bites or two stingers.
        // Future refactor of the `CompoundAttack` chassis shouldn't strip
        // the mixed-limb pairing.
        //
        // Read off the compound's declared limbs rather than counted out
        // of a resolved swing. The effect count cannot express this: a
        // stinger that hits and whose save fails pushes *two* payloads
        // (the piercing and the venom), so "at most two effects" is not
        // the shape of one bite and one stinger — it is the shape of a
        // fight in which something missed, which is a fact about the
        // dice rather than about the stat block.
        let parts: Vec<(&str, u32)> = PURPLE_WORM_MULTI
            .parts
            .iter()
            .map(|(action, count)| (action.name(), *count))
            .collect();
        assert_eq!(
            parts,
            vec![("purple worm bite", 1), ("purple worm tail stinger", 1)],
            "the worm bites once and stings once"
        );
    }
}
