use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{BONE_DEVIL_CLAWS, BONE_DEVIL_MULTI, BONE_DEVIL_STING};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense,
};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Bone Devil — CR 9 fiend. Large-footprint flying devil with a
/// venom-tipped scorpion tail. Three attack lanes:
/// - **Sting**: 2d8+STR piercing + CON-save 5d6 poison rider that also
///   installs Poisoned for 10 rounds on fail. The devil's signature
///   one-two punch.
/// - **Claws**: 1d8+STR slashing melee, reach 1 — the secondary attack
///   lane that pairs into the multiattack.
/// - **Multiattack** (2 claws + 1 sting): heterogeneous compound; the
///   devil's full opening salvo.
///
/// Stats target the MM Bone Devil block: 142 HP, AC 19, STR 18 / DEX 16
/// / CON 18 / WIS 14 / CHA 16. Standard devil envelope: immune to fire +
/// poison, resistant to cold + mundane B/P/S; can't be poisoned or
/// charmed. Proficient INT / WIS / CHA saves (the mental lane).
pub static BONE_DEVIL_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&BONE_DEVIL_CLAWS);
    actions.push(&*BONE_DEVIL_STING);
    actions.push(&*BONE_DEVIL_MULTI);
    CreatureTemplate {
        name: "Bone Devil",
        // 'd' (lowercase) is free; capital 'D' is Doppelganger; capital
        // 'B' is Bracers loot — creature glyph 'b' for the bone devil
        // is open.
        glyph: 'b',
        ac: 19,
        // 15d10+60 ≈ 142 average per the MM Bone Devil stat block.
        hitpoints: "15d10+60".parse().unwrap(),
        speed: 40., // flying speed 40 ft RAW
        strength: 18,
        intelligence: 13,
        dexterity: 16,
        wisdom: 14,
        constitution: 18,
        charisma: 16,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(120)]),
        languages: HashSet::from([Language::Infernal, Language::Common]),
        cr: 9.0,
        size: Size::Large,
        creature_type: CreatureType::Fiend,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        // MM Bone Devil: immune to fire + poison; resistant to cold +
        // non-magical B/P/S. We omit the magical-vs-mundane distinction.
        damage_modifiers: HashMap::from([
            (DamageType::Fire, DamageModifier::Immunity),
            (DamageType::Poison, DamageModifier::Immunity),
            (DamageType::Cold, DamageModifier::Resistance),
        ]),
        // MM Bone Devil proficient saves: INT / WIS / CHA (the mental
        // lane — bone devils are mid-tier devils with strong saves).
        proficient_saves: HashSet::from([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]),
        // Devil envelope: can't be poisoned (an immunity granted by
        // their fiendish constitution).
        condition_immunities: HashSet::from([Condition::Poisoned]),
        features: HashSet::new(),
        regen_per_round: 0,
        regen_suppressors: HashSet::new(),
        legendary_resistances: 0,
        has_evasion: false,
        has_uncanny_dodge: false,
        has_displacement: false,
        has_danger_sense: false,
        has_pack_tactics: false,
        has_magic_resistance: true,
        recharge_abilities: Vec::new(),
        legendary_actions_per_round: 0,
        has_extra_attack: false,
    }
});
