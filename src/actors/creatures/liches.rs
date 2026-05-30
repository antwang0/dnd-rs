use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::LICH_PARALYZING_TOUCH;
use crate::actions::spells::{
    BANISHMENT, BESTOW_CURSE, CHILL_TOUCH, CLOUDKILL, CONE_OF_COLD, COUNTERSPELL, DISINTEGRATE,
    FINGER_OF_DEATH, FIREBALL, FIRE_BOLT, HOLD_MONSTER, ICE_STORM, LIGHTNING_BOLT, MAGIC_MISSILE,
    MIND_SLIVER, MIRROR_IMAGE, POWER_WORD_KILL, POWER_WORD_STUN, SCORCHING_RAY, SHIELD,
    SYNAPTIC_STATIC, TOLL_THE_DEAD, VAMPIRIC_TOUCH,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense,
};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Lich — CR 21 undead spellcaster. The marquee boss caster: huge spell
/// list, paralyzing touch as a fallback melee, and a wide envelope of
/// damage / condition immunities. Spells span the entire wizard list at
/// boss-tier slot counts (4/3/3/3/3/2/2/2/2 — late-game archmage with
/// double-coverage on the killer level-7/8/9 lane). Doesn't roll death
/// saves (creature, not PC), but Toll the Dead + Chill Touch keep the
/// at-will damage rolling even after the slot pool drains.
pub static LICH_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*LICH_PARALYZING_TOUCH);
    // Cantrips — free damage option once slots run dry.
    actions.push(&*FIRE_BOLT);
    actions.push(&*CHILL_TOUCH);
    actions.push(&*MIND_SLIVER);
    actions.push(&*TOLL_THE_DEAD);
    // Level-1 / 2 / 3 — clean ramp from MM and shield is the iconic
    // reaction-budget defense.
    actions.push(&*SHIELD);
    actions.push(&*COUNTERSPELL);
    actions.push(&*MAGIC_MISSILE);
    actions.push(&*MIRROR_IMAGE);
    actions.push(&*SCORCHING_RAY);
    actions.push(&*VAMPIRIC_TOUCH);
    actions.push(&*FIREBALL);
    actions.push(&*LIGHTNING_BOLT);
    actions.push(&*BESTOW_CURSE);
    // Level 4 / 5 — control + damage curve.
    actions.push(&*ICE_STORM);
    actions.push(&*BANISHMENT);
    actions.push(&*HOLD_MONSTER);
    actions.push(&*CLOUDKILL);
    actions.push(&*CONE_OF_COLD);
    // Level 6 / 7 / 8 — finishing power.
    actions.push(&*DISINTEGRATE);
    actions.push(&*FINGER_OF_DEATH);
    actions.push(&*SYNAPTIC_STATIC);
    actions.push(&*POWER_WORD_STUN);
    // Level 9 — the lich's signature panic button.
    actions.push(&*POWER_WORD_KILL);
    // Necromancy-themed additions matching the lich's archetype:
    //   - lv4 **Blight**: 8d8 necrotic single-target nuke (necromancy
    //     against fleshy threats — a defining lich spell RAW).
    //   - lv6 **Circle of Death**: 8d6 necrotic 30ft-radius AoE
    //     (the lich's signature mass-necrotic option).
    //   - lv7 **Delayed Blast Fireball**: 12d6 fire DEX-save burst
    //     (rounds out the lich's lv7 AoE lane alongside Finger of Death).
    //   - lv8 **Incendiary Cloud**: 10d8 fire neutral-burst (mass AoE
    //     at lv8 next to Synaptic Static / Power Word Stun).
    //   - lv9 **Weird**: 10d10 psychic + Frightened — usually shrugged
    //     off by Lich's own Frightened immunity, but lethal against
    //     a party that's not equipped to resist illusion-fear.
    actions.push(&*crate::actions::spells::BLIGHT);
    actions.push(&*crate::actions::spells::CIRCLE_OF_DEATH);
    actions.push(&*crate::actions::spells::DELAYED_BLAST_FIREBALL);
    actions.push(&*crate::actions::spells::INCENDIARY_CLOUD);
    actions.push(&*crate::actions::spells::WEIRD);
    actions.push(&*crate::actions::spells::DOMINATE_MONSTER);
    actions.push(&*crate::actions::spells::PLANE_SHIFT);
    CreatureTemplate {
        name: "Lich",
        // 'L' is taken in some content; use 'l' (lowercase L) for lich.
        glyph: 'l',
        ac: 17,
        // 18d8+54 = 135 average per MM CR 21.
        hitpoints: "18d8+54".parse().unwrap(),
        speed: 30.,
        strength: 11,
        intelligence: 20, // primary spellcasting ability
        dexterity: 16,
        wisdom: 14,
        constitution: 16,
        charisma: 16,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Truesight(120), SpecialSense::Darkvision(120)]),
        languages: HashSet::from([Language::Common, Language::Draconic, Language::Infernal]),
        cr: 21.0,
        size: Size::Medium,
        creature_type: CreatureType::Undead,
        actions,
        // Boss-tier loadout: 4/3/3/3/3/2/2/2/2.
        spell_slots_by_level: vec![4, 3, 3, 3, 3, 2, 2, 2, 2],
        rolls_death_saves: false,
        // 5e Lich: necrotic / poison immunity, resistance to cold /
        // lightning / non-magical physical (we use straight resistance
        // for the three physical types to keep parity with the rest of
        // our undead pool).
        damage_modifiers: HashMap::from([
            (DamageType::Necrotic, DamageModifier::Immunity),
            (DamageType::Poison, DamageModifier::Immunity),
            (DamageType::Cold, DamageModifier::Resistance),
            (DamageType::Lightning, DamageModifier::Resistance),
            (DamageType::Bludgeoning, DamageModifier::Resistance),
            (DamageType::Piercing, DamageModifier::Resistance),
            (DamageType::Slashing, DamageModifier::Resistance),
        ]),
        // Lich save profile: prof in CON / INT / WIS (Legendary Resistance-
        // adjacent in 5e RAW, but we approximate with proficient saves).
        proficient_saves: HashSet::from([
            AbilityScoreType::Constitution,
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
        ]),
        // Standard undead condition immunities + Frightened / Paralyzed
        // (a lich's mind doesn't break under fear or paralysis effects).
        condition_immunities: HashSet::from([
            Condition::Poisoned,
            Condition::Charmed,
            Condition::Frightened,
            Condition::Paralyzed,
            Condition::Exhausted,
        ]),
        features: HashSet::new(),
        regen_per_round: 0,
        regen_suppressors: HashSet::new(),
        // 5e Legendary Resistance (3/Day) — RAW per MM. The lich's
        // signature defense against the party's save-or-die / save-or-
        // suck spells (Hold Monster, Banishment, Power Word Stun).
        legendary_resistances: 3,
        has_evasion: false,
        has_uncanny_dodge: false,
        has_displacement: false,
        has_danger_sense: false,
        has_pack_tactics: false,
        has_magic_resistance: true,
        recharge_abilities: Vec::new(),
        legendary_actions_per_round: 3,
        has_extra_attack: false,
        brutal_critical_dice: 0,
        crit_threshold: 20,
        has_lucky: false,
        has_aura_of_protection: false,
        has_aura_of_courage: false,
    }
});
