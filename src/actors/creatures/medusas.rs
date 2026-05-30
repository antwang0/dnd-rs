use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    LONGBOW, MEDUSA_MULTI, MEDUSA_PETRIFYING_GAZE, MEDUSA_SNAKE_HAIR,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Medusa — CR 6 monstrosity. The iconic petrifying-gaze threat:
/// - **Multiattack** (snake hair + petrifying gaze in one Action). The
///   gaze forces a CON save vs DC 14; on fail the target is Petrified
///   for 1 round (we cap at 1 round so a single hit doesn't game-over;
///   the action-economy lockout while Petrified is brutal enough).
/// - **Snake Hair** (standalone): 1d4+DEX piercing + 4d6 poison rider.
/// - **Petrifying Gaze** (standalone): the pure stone-lock save.
/// - **Longbow**: ranged piercing option for when the gaze is on cooldown.
///
/// MM RAW: AC 15, ~127 HP (17d8+51), DEX 15, CON 16. No damage or
/// condition immunities (medusas are mortal monstrosities). Darkvision
/// 60. Speaks Common in this version (RAW says "the languages it knew
/// in life" — we tag Common as the baseline).
pub static MEDUSA_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*MEDUSA_MULTI);
    actions.push(&*MEDUSA_SNAKE_HAIR);
    actions.push(&*MEDUSA_PETRIFYING_GAZE);
    actions.push(&LONGBOW);
    CreatureTemplate {
        name: "Medusa",
        // 'd' (lowercase) — distinct from 'M' (Wizard/Mage). Reads as
        // a smaller-statured humanoid with the snake-hair flourish.
        glyph: 'd',
        ac: 15,
        // 17d8+51 ≈ 127 average per MM (CR 6).
        hitpoints: "17d8+51".parse().unwrap(),
        speed: 30.,
        strength: 10,
        intelligence: 12,
        dexterity: 15,
        wisdom: 13,
        constitution: 16,
        charisma: 15,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common]),
        cr: 6.0,
        size: Size::Medium,
        creature_type: CreatureType::Monstrosity,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        damage_modifiers: HashMap::new(),
        proficient_saves: HashSet::new(),
        // Medusae avert their own gaze — RAW doesn't gate this on the
        // condition lane (they have no immunities). Left empty.
        condition_immunities: HashSet::new(),
        features: HashSet::new(),
        regen_per_round: 0,
        regen_suppressors: HashSet::new(),
        legendary_resistances: 0,
        has_evasion: false,
        has_uncanny_dodge: false,
        has_displacement: false,
        has_danger_sense: false,
        has_pack_tactics: false,
        has_magic_resistance: false,
        recharge_abilities: Vec::new(),
        legendary_actions_per_round: 0,
        has_extra_attack: false,
        brutal_critical_dice: 0,
        crit_threshold: 20,
        has_lucky: false,
        has_aura_of_protection: false,
        has_aura_of_courage: false,
    }
});
