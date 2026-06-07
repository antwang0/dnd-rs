use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::DAGGER;
use crate::actions::spells::{
    ACID_SPLASH, ANIMATE_DEAD, ARMOR_OF_AGATHYS, BANISHMENT, BESTOW_CURSE, BLINDNESS,
    BURNING_HANDS, CHARM_PERSON, CHILL_TOUCH, COUNTERSPELL, DIMENSION_DOOR, ELDRITCH_BLAST,
    EYEBITE, FEAR, FIRE_BOLT, FLY, HELLISH_REBUKE, HEX, HOLD_MONSTER, HOLD_PERSON,
    HYPNOTIC_PATTERN, INVISIBILITY, LIGHTNING_LURE, MAGE_ARMOR, MISTY_STEP, POISON_SPRAY,
    POWER_WORD_KILL, POWER_WORD_STUN, SHIELD, SICKENING_RADIANCE, SLEEP, SUGGESTION,
    VAMPIRIC_TOUCH, WITCH_BOLT,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Warlock PC template. CHA-primary half-caster with Pact Magic — RAW
/// the warlock's defining feature is short-rest spell slots: a small
/// pool (~2-4) that all sit at the warlock's highest available slot
/// level, refreshing on a short rest. The engine doesn't model short
/// rests as a discrete event today (long rest is the only refresh
/// trigger), so we approximate with a flat 4 slots concentrated at
/// level 5 — the load-bearing apex slot the warlock blasts with — plus
/// a thin lower-level spread for situational picks.
///
/// Loadout philosophy:
/// - **Eldritch Blast** as the at-will ranged cantrip (the warlock's
///   signature cantrip, scales with caster level).
/// - **Hex** as the per-encounter rider buff (concentration; pairs with
///   EB swings).
/// - **Hellish Rebuke** as the bonus-action reactive damage.
/// - **Witch Bolt** as the lv1 sustained zap (concentration).
/// - **Hold Person / Suggestion / Hypnotic Pattern** as enchantment
///   control.
/// - **Eyebite / Power Word Kill** as the apex single-target threats.
///
/// Stat shape: AC 12 (unarmored + DEX), 8d8+8 HP (~44), CHA 18.
/// Proficient WIS + CHA saves (RAW). Speaks Common + Infernal (the
/// patron's tongue).
pub static WARLOCK_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&DAGGER);
    // Cantrips (at-will)
    actions.push(&*ELDRITCH_BLAST);
    actions.push(&*FIRE_BOLT);
    actions.push(&*CHILL_TOUCH);
    actions.push(&*ACID_SPLASH);
    actions.push(&*POISON_SPRAY);
    actions.push(&*LIGHTNING_LURE);
    // Level 1 — Hex defines the warlock's rider loop; Hellish Rebuke
    // for reactive burst; Witch Bolt for sustained zap; Mage Armor /
    // Shield for survivability; Charm / Sleep for soft control.
    actions.push(&*HEX);
    actions.push(&*HELLISH_REBUKE);
    actions.push(&*WITCH_BOLT);
    actions.push(&*MAGE_ARMOR);
    actions.push(&*SHIELD);
    actions.push(&*CHARM_PERSON);
    actions.push(&*SLEEP);
    actions.push(&*BURNING_HANDS);
    // Armor of Agathys — the warlock's signature self-buff: 5 temp HP
    // plus 5 cold damage reflected on melee hit. Pairs with the
    // warlock's lv1 slot economy as a pre-fight tank-up.
    actions.push(&*ARMOR_OF_AGATHYS);
    // Level 2 — Misty Step (escape), Hold Person (control), Invisibility,
    // Blindness, Suggestion (single-target charm).
    actions.push(&*MISTY_STEP);
    actions.push(&*HOLD_PERSON);
    actions.push(&*INVISIBILITY);
    actions.push(&*BLINDNESS);
    actions.push(&*SUGGESTION);
    // Level 3 — Fear (cone Frightened), Counterspell (anti-caster),
    // Hypnotic Pattern (AoE charm), Vampiric Touch (sustained life
    // drain), Bestow Curse (single-target debuff), Fly, Animate Dead.
    actions.push(&*FEAR);
    actions.push(&*COUNTERSPELL);
    actions.push(&*HYPNOTIC_PATTERN);
    actions.push(&*VAMPIRIC_TOUCH);
    actions.push(&*BESTOW_CURSE);
    actions.push(&*FLY);
    actions.push(&*ANIMATE_DEAD);
    // Level 4 — Banishment (single-target removal), Dimension Door
    // (teleport). Sickening Radiance: enemy-only 30ft burst with
    // Exhausted-on-fail; fits the warlock's "control burst" niche.
    actions.push(&*BANISHMENT);
    actions.push(&*DIMENSION_DOOR);
    actions.push(&*SICKENING_RADIANCE);
    // Level 5 — Hold Monster (single-target paralysis on a bigger fish).
    actions.push(&*HOLD_MONSTER);
    // Negative Energy Flood — necromancy lv5 burst that fits the
    // patron's flavor; CON-save halve, 5d12 necrotic on fail.
    actions.push(&*crate::actions::spells::NEGATIVE_ENERGY_FLOOD);
    // Level 6 — Eyebite (single-target sleep), the warlock's apex
    // control. RAW gates Eyebite at lv6; we put it at lv6 here too.
    actions.push(&*EYEBITE);
    // Level 9 — Power Word Stun and Power Word Kill (single-target
    // boss-killers; the warlock's apex damage button).
    actions.push(&*POWER_WORD_STUN);
    actions.push(&*POWER_WORD_KILL);
    // Newest warlock additions:
    //   - cantrip **Thunderclap**: self-centered CON-save burst.
    //   - lv2 **Mind Spike**: single-target psychic save-for-half.
    //   - lv4 **Psychic Lance**: psychic save-for-half + Incapacitated
    //     rider on fail. Pair with Hex for a +1d6 necrotic rider on the
    //     base damage.
    actions.push(&*crate::actions::spells::THUNDERCLAP);
    actions.push(&*crate::actions::spells::MIND_SPIKE);
    actions.push(&*crate::actions::spells::PSYCHIC_LANCE);
    // Latest warlock additions (lv0-1):
    //   - cantrip **Sword Burst**: 1-tile force burst around caster —
    //     a melee-flavored at-will for warlocks who close into reach.
    //   - cantrip **Blade Ward**: self damage-resistance till next turn
    //     (rare defensive cantrip option for the squishy chassis).
    //   - lv1 **Fog Cloud**: concentration heavy-obscurement burst.
    actions.push(&*crate::actions::spells::SWORD_BURST);
    actions.push(&*crate::actions::spells::BLADE_WARD);
    actions.push(&*crate::actions::spells::FOG_CLOUD);
    // Signature warlock pickup:
    //   - lv1 **Arms of Hadar** (warlock-only): self-centered necrotic
    //     burst with STR save for half + no-reactions rider on fail.
    //     Punishes melee swarms that close on the warlock.
    actions.push(&*crate::actions::spells::ARMS_OF_HADAR);
    // lv3 **Hunger of Hadar** — warlock signature: cold + acid sphere.
    actions.push(&*crate::actions::spells::HUNGER_OF_HADAR);
    // lv5 **Eldritch Smite** — warlock melee burst + prone on fail.
    actions.push(&*crate::actions::spells::ELDRITCH_SMITE);
    // Telekinetic — cantrip bonus-action shove. 5ft pull on a failed
    // STR save, no slot. Cheap repositioning for the warlock's
    // bonus-action lane (otherwise empty between Hex / Hex re-target).
    actions.push(&*crate::actions::spells::TELEKINETIC);
    // Green-Flame Blade — cantrip CHA-scaled melee touch (1d8 fire +
    // ability-modifier fire leap to the lowest-HP adjacent enemy on a
    // hit). Pact-of-the-Blade-style melee cantrip for the warlock —
    // pairs with Eldritch Blast for a melee-vs-ranged at-will lane.
    actions.push(&*crate::actions::spells::GREEN_FLAME_BLADE);
    actions.push(&*crate::actions::spells::THUNDER_STEP);
    actions.push(&*crate::actions::spells::SHADOW_BLADE);
    actions.push(&*crate::actions::spells::CLOUD_OF_DAGGERS);
    // Warlock capstones:
    //   - lv6 **Flesh to Stone** (transmutation): CON save vs the
    //     warlock's spell DC; on fail target is Petrified for ~1 minute
    //     (concentration-bound). Single-target lockdown lane that
    //     complements Eyebite (WIS save, Asleep) at the same slot tier —
    //     the warlock can pick whichever save the target is weakest at.
    //   - lv9 **Psychic Scream** (enchantment): self-centered 8-tile
    //     burst, 14d6 psychic INT-save for half + Stunned-on-fail. The
    //     warlock's lv9 mass-control button — burst stuns the whole
    //     hostile back rank in one tap, distinct from Power Word Kill
    //     (HP-gated single-target) and Power Word Stun (HP-gated single
    //     stun).
    actions.push(&*crate::actions::spells::FLESH_TO_STONE);
    actions.push(&*crate::actions::spells::PSYCHIC_SCREAM);
    CreatureTemplate {
        name: "Warlock",
        // 'L' (uppercase) — distinct from 'l' (Lich), 'W' (Wolf glyph),
        // 'M' (Wizard / Mage). Reads as a robed CHA-caster.
        glyph: 'L',
        ac: 12,
        hitpoints: "8d8+8".parse().unwrap(),
        speed: 30.,
        strength: 8,
        intelligence: 12,
        dexterity: 14,
        wisdom: 12,
        constitution: 14,
        charisma: 18, // primary spellcasting ability
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        languages: HashSet::from([Language::Common, Language::Infernal]),
        cr: 4.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        // Pact Magic compromise: a flat 4 lv5 slots (the warlock's
        // top-level slots all sit at the highest available slot level
        // RAW). Lower levels carry just 1 slot apiece so the situational
        // picks (Misty Step, Counterspell, etc.) still have ammunition
        // without diluting the "blast with the apex slot" feel.
        // Index: lv1=2, lv2=1, lv3=1, lv4=1, lv5=4, lv6+1 (Eyebite),
        // lv7=0, lv8=0, lv9=1 (Power Word Kill / Stun).
        spell_slots_by_level: vec![2, 1, 1, 1, 4, 1, 0, 0, 1],
        rolls_death_saves: true,
        damage_modifiers: HashMap::new(),
        // Warlocks are proficient in WIS and CHA saves (5e PHB).
        proficient_saves: HashSet::from([
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]),
        condition_immunities: HashSet::new(),
        // 5e Warlock Eldritch Invocations:
        //   - Agonizing Blast: +CHA mod to each Eldritch Blast beam.
        //   - Repelling Blast: 10ft (4-tile) push on hit, Large-or-
        //     smaller targets only.
        //   - Eldritch Mind: advantage on Constitution saves to maintain
        //     concentration (read at the damage chokepoint).
        // RAW a level-5 warlock picks 3 invocations; the kit pre-picks
        // these three since EB is the signature cantrip and
        // concentration-bound spells (Hex / Hunger of Hadar) form the
        // back half of the warlock's lockdown plan. Permanent passive
        // features — never consumed; the relevant cast / save sites
        // read them via `feature_available`.
        features: HashSet::from([
            crate::actions::class_features::AGONIZING_BLAST_TAG,
            crate::actions::class_features::REPELLING_BLAST_TAG,
            crate::actions::class_features::ELDRITCH_MIND_TAG,
        ]),
        regen_per_round: 0,
        regen_suppressors: HashSet::new(),
        legendary_resistances: 0,
        has_evasion: false,
        has_uncanny_dodge: false,
        has_deflect_missiles: false,
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
        has_brave: false,
        has_fey_ancestry: false,
        has_aura_of_protection: false,
        has_aura_of_courage: false,
        has_savage_attacks: false,
        has_dwarven_resilience: false,
        has_gnome_cunning: false,
        draconic_ancestry: None,
        sorcery_points: 0,
    }
});

