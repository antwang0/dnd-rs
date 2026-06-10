/// Flat sum of every passive bonus an actor gets from carried items.
/// Each `ActorInstance` aggregates one of these on demand from its
/// inventory. Keeping this a single struct (rather than per-stat lookups)
/// means stat accessors fold in item bonuses with one method call instead
/// of N inventory walks.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ItemBonuses {
    pub ac: i32,
    pub max_hp: i32,
    /// Speed bonus is integer feet (5e items always grant whole-foot
    /// values like +10 boots). Stored as i32 to allow future debuffs.
    pub speed: i32,
    /// Flat bonus added to every saving throw modifier.
    pub save: i32,
    /// Flat bonus added to every attack roll the holder makes (weapon
    /// and spell attacks alike — folded into `caster_attack_buffs` so
    /// the install-side `attack_bonus_buff` lane is shared with item
    /// passives). Matches the `+1 weapon` / Bracers of Archery loot
    /// archetype. 5e RAW: a +1 weapon adds to both attack AND damage
    /// rolls — we ride the attack half here; the damage half lives on
    /// `damage_bonus` below so weapon swings AND spell attacks both
    /// see the bonus once (no double-dipping). 0 by default — most
    /// trinkets leave this alone.
    pub attack_bonus: i32,
    /// Flat bonus added to every damage roll the holder lands on a hit
    /// (weapon and spell attacks alike). Used by `+1 weapon`-style items
    /// to grant the RAW "+N to attack AND damage" pair. Read at the
    /// damage-roll site in `engine::attack` / `spells.rs`'s spell-attack
    /// chokepoint. 0 by default.
    pub damage_bonus: i32,
}

impl ItemBonuses {
    /// All-zero baseline. Use as the tail of a struct-update literal so
    /// an item that only bumps one stat doesn't enumerate the other zero
    /// fields — `ItemBonuses { ac: 1, ..ItemBonuses::ZERO }` reads cleaner
    /// than spelling out every neutral field. Const-evaluable so static
    /// items can build off it.
    pub const ZERO: ItemBonuses = ItemBonuses {
        ac: 0,
        max_hp: 0,
        speed: 0,
        save: 0,
        attack_bonus: 0,
        damage_bonus: 0,
    };
}

impl std::ops::Add for ItemBonuses {
    type Output = ItemBonuses;
    fn add(self, other: ItemBonuses) -> ItemBonuses {
        ItemBonuses {
            ac: self.ac + other.ac,
            max_hp: self.max_hp + other.max_hp,
            speed: self.speed + other.speed,
            save: self.save + other.save,
            attack_bonus: self.attack_bonus + other.attack_bonus,
            damage_bonus: self.damage_bonus + other.damage_bonus,
        }
    }
}

/// A piece of equipment. Items split implicitly into "passive trinket"
/// (only `bonuses` populated, `on_use = None`) and "consumable"
/// (`on_use` references a static action). Consumables don't grant
/// passive bonuses today — if a future item needs both, it just sets
/// both fields. Items are referenced via `&'static Item` so cloning an
/// inventory is cheap and definitions stay single-sourced.
#[derive(Clone, Copy)]
pub struct Item {
    pub name: &'static str,
    /// Map glyph for ground rendering. Convention: a single visible ASCII
    /// character that doesn't collide with terrain (`░` `█`) or creatures
    /// (uppercase letters).
    pub glyph: char,
    pub bonuses: ItemBonuses,
    /// If `Some`, the actor carrying this item gets this action added to
    /// their available-actions list. Using the action consumes one copy
    /// of this item (action's own logic handles the removal). `None` for
    /// passive-only trinkets like rings and cloaks.
    pub on_use: Option<&'static (dyn crate::actions::action_template::Action + Send + Sync)>,
    /// Conditions the wearer is immune to while carrying this item.
    /// Folded into `ActorInstance::effectively_immune_to_condition` so
    /// trinkets like the Necklace of Adaptation (Poisoned-immune) and
    /// Ring of Free Action (Paralyzed / Restrained / Grappled-immune)
    /// fall out of the same install-gate that already handles template
    /// and dynamic immunities. Empty for items that don't grant
    /// condition immunities (the default for most loot).
    pub condition_immunities: &'static [crate::conditions::Condition],
    /// Damage types the wearer is resistant to while carrying this item.
    /// Folded into `ActorInstance::effective_damage` so trinkets like the
    /// Brooch of Shielding (force-resistant) and Boots of the Winterlands
    /// (cold-resistant) halve incoming damage through the same lane that
    /// already handles template and condition-based resistance. Honors
    /// the 5e "one halving" stacking rule — item resistance won't
    /// re-halve damage that's already been halved by a template or
    /// condition source. Empty for items without typed resistance.
    pub damage_resistances: &'static [crate::engine::types::DamageType],
    /// Damage types the wearer is fully immune to while carrying this
    /// item. Folded into `ActorInstance::effective_damage` so trinkets
    /// like the Periapt of Proof against Poison (poison-immune) zero
    /// incoming damage through the same lane that already handles
    /// template-level immunity. Immunity wins over everything: an item
    /// immunity short-circuits the damage pipeline before resistance /
    /// vulnerability rolls fire. Empty for items without typed immunity.
    pub damage_immunities: &'static [crate::engine::types::DamageType],
}

impl Item {
    /// All-empty / no-op defaults for the rare fields. Use as the tail of
    /// a struct-update literal (`Item { name: "...", ..Item::DEFAULTS }`)
    /// so items that don't grant condition immunities / damage
    /// resistances / immunities don't have to repeat the three empty-slice
    /// fields at every definition. `name`, `glyph`, and `bonuses` should
    /// always be overridden — the defaults here are just type-correct
    /// placeholders so the struct literal is total. Const-evaluable so it
    /// works in `static` initializers.
    pub const DEFAULTS: Item = Item {
        name: "",
        glyph: ' ',
        bonuses: ItemBonuses::ZERO,
        on_use: None,
        condition_immunities: &[],
        damage_resistances: &[],
        damage_immunities: &[],
    };
}

pub static RING_OF_PROTECTION: Item = Item {
    name: "Ring of Protection",
    glyph: '=',
    bonuses: ItemBonuses { ac: 1, save: 1, ..ItemBonuses::ZERO },
    ..Item::DEFAULTS
};

pub static BOOTS_OF_STRIDING: Item = Item {
    name: "Boots of Striding",
    glyph: 'b',
    bonuses: ItemBonuses { speed: 10, ..ItemBonuses::ZERO },
    ..Item::DEFAULTS
};

pub static CLOAK_OF_RESISTANCE: Item = Item {
    name: "Cloak of Resistance",
    glyph: 'c',
    bonuses: ItemBonuses { save: 2, ..ItemBonuses::ZERO },
    ..Item::DEFAULTS
};

pub static AMULET_OF_HEALTH: Item = Item {
    name: "Amulet of Health",
    glyph: 'a',
    bonuses: ItemBonuses { max_hp: 10, ..ItemBonuses::ZERO },
    ..Item::DEFAULTS
};

/// Headband of Insight — minor caster-flavor trinket. +1 save bonus,
/// no AC or speed. Distinct loot tier from Cloak of Resistance (which
/// gives +2) so the loot pool has stratified strength.
pub static HEADBAND_OF_INSIGHT: Item = Item {
    name: "Headband of Insight",
    glyph: 'h',
    bonuses: ItemBonuses { save: 1, ..ItemBonuses::ZERO },
    ..Item::DEFAULTS
};

/// Bracers of Defense — light AC bump. Cheaper loot than Ring of
/// Protection (which gives +1 AC and +1 save), giving the LOOT_POOL
/// a clearer common / uncommon ladder.
pub static BRACERS_OF_DEFENSE: Item = Item {
    name: "Bracers of Defense",
    glyph: 'B',
    bonuses: ItemBonuses { ac: 1, ..ItemBonuses::ZERO },
    ..Item::DEFAULTS
};

pub static POTION_OF_HEALING: Item = Item {
    name: "Potion of Healing",
    glyph: 'p',
    on_use: Some(&crate::actions::item_actions::DRINK_HEALING_POTION),
    ..Item::DEFAULTS
};

pub static POTION_OF_GREATER_HEALING: Item = Item {
    name: "Potion of Greater Healing",
    glyph: 'P',
    on_use: Some(&crate::actions::item_actions::DRINK_GREATER_HEALING_POTION),
    ..Item::DEFAULTS
};

pub static SCROLL_OF_FIREBALL: Item = Item {
    name: "Scroll of Fireball",
    glyph: 's',
    on_use: Some(&crate::actions::item_actions::READ_FIREBALL_SCROLL),
    ..Item::DEFAULTS
};

pub static SCROLL_OF_MAGIC_MISSILE: Item = Item {
    name: "Scroll of Magic Missile",
    glyph: 'm',
    on_use: Some(&crate::actions::item_actions::READ_MAGIC_MISSILE_SCROLL),
    ..Item::DEFAULTS
};

/// Cloak of Protection — premium passive trinket. +1 AC AND +1 to all
/// saves. Strictly better than Cloak of Resistance for tanks who need
/// the AC bump; rarer in the loot pool.
pub static CLOAK_OF_PROTECTION: Item = Item {
    name: "Cloak of Protection",
    glyph: 'C',
    bonuses: ItemBonuses { ac: 1, save: 1, ..ItemBonuses::ZERO },
    ..Item::DEFAULTS
};

/// Shield — passive +2 AC, no save bonus. Classic light-armor pairing
/// with one-handed weapons. Distinct loot tier from heavy armor since
/// we don't model armor proficiency yet.
pub static SHIELD: Item = Item {
    name: "Shield",
    glyph: 'S',
    bonuses: ItemBonuses { ac: 2, ..ItemBonuses::ZERO },
    ..Item::DEFAULTS
};

/// Antitoxin — single-use consumable. Drinking removes the Poisoned
/// condition and grants advantage on the next CON save against poison
/// (modeled as a flat +5 save buff via Bless's mechanic). One-shot:
/// the action removes the item from inventory after use.
pub static ANTITOXIN: Item = Item {
    name: "Antitoxin",
    glyph: 'A',
    on_use: Some(&crate::actions::item_actions::DRINK_ANTITOXIN),
    ..Item::DEFAULTS
};

/// Potion of Speed — bonus action; gain an extra Action this turn plus
/// a +1 attack/save buff (a simplified Haste). Single-use consumable;
/// the buff clears on long rest with the rest of the buff state.
pub static POTION_OF_SPEED: Item = Item {
    name: "Potion of Speed",
    glyph: '!',
    on_use: Some(&crate::actions::item_actions::DRINK_POTION_OF_SPEED),
    ..Item::DEFAULTS
};

/// Potion of Heroism — bonus action; grants 10 temp HP and the Heroic
/// condition (Frightened immunity + temp HP regen tagged onto the
/// buff for 10 rounds). Single-use consumable; the buff drops with
/// the condition timer.
pub static POTION_OF_HEROISM: Item = Item {
    name: "Potion of Heroism",
    glyph: 'H',
    on_use: Some(&crate::actions::item_actions::DRINK_POTION_OF_HEROISM),
    ..Item::DEFAULTS
};

/// Potion of Invisibility — action; grants the Invisible condition for
/// 10 rounds (attacks vs holder at disadvantage, holder's attacks at
/// advantage). Single-use consumable; the buff drops with the
/// condition timer.
pub static POTION_OF_INVISIBILITY: Item = Item {
    name: "Potion of Invisibility",
    glyph: 'i',
    on_use: Some(&crate::actions::item_actions::DRINK_POTION_OF_INVISIBILITY),
    ..Item::DEFAULTS
};

/// Periapt of Wound Closure — +5 max HP passive trinket. Thematic
/// flavor: stabilizes a dying wearer (modeled as extra HP cushion).
pub static PERIAPT_OF_WOUND_CLOSURE: Item = Item {
    name: "Periapt of Wound Closure",
    glyph: '+',
    bonuses: ItemBonuses { max_hp: 5, ..ItemBonuses::ZERO },
    ..Item::DEFAULTS
};

/// Gauntlets of Ogre Power — +1 AC from the reinforced plates on the
/// gauntlets, plus +5 max HP from the magical vigor. A martial
/// trinket that makes the front-liner stickier.
pub static GAUNTLETS_OF_OGRE_POWER: Item = Item {
    name: "Gauntlets of Ogre Power",
    glyph: 'G',
    bonuses: ItemBonuses { ac: 1, max_hp: 5, ..ItemBonuses::ZERO },
    ..Item::DEFAULTS
};

/// Scroll of Lightning Bolt — one-shot 8d6 lightning burst along a
/// line. Uses the same mechanics as Fireball scroll but typed lightning.
pub static SCROLL_OF_LIGHTNING_BOLT: Item = Item {
    name: "Scroll of Lightning Bolt",
    glyph: 'l',
    on_use: Some(&crate::actions::item_actions::READ_LIGHTNING_BOLT_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Cure Wounds — single-target touch heal (2d8+2 HP). Fills
/// the "single-target heal scroll" niche between the self-only Potion
/// of Healing (2d4+2) and the spell Cure Wounds (caster-mod scaling).
/// Heals a touch-range target on use; consumed on use.
pub static SCROLL_OF_CURE_WOUNDS: Item = Item {
    name: "Scroll of Cure Wounds",
    glyph: 'w',
    on_use: Some(&crate::actions::item_actions::READ_CURE_WOUNDS_SCROLL),
    ..Item::DEFAULTS
};

/// Stone of Good Luck (Luckstone) — premium passive trinket. +1 to all
/// saving throws AND +1 to AC. 5e RAW grants a +1 luck bonus to ability
/// checks and saving throws while carried; we collapse the ability-check
/// half onto the save lane (the engine routes the most consequential
/// rolls — concentration, save-or-suck — through the save path), and
/// throw in a +1 AC as flavor for the luck shielding the holder from
/// blows. Distinct loot tier from the Cloak of Protection (also +1 / +1)
/// — same numeric profile, but priced as a separate roll so the loot
/// pool doesn't collapse to one premium passive.
pub static STONE_OF_GOOD_LUCK: Item = Item {
    name: "Stone of Good Luck",
    glyph: 'L',
    bonuses: ItemBonuses { ac: 1, save: 1, ..ItemBonuses::ZERO },
    ..Item::DEFAULTS
};

/// Necklace of Adaptation — passive trinket. Grants immunity to the
/// Poisoned condition while worn. 5e RAW: "you are immune to harmful
/// gases" plus you can breathe freely; we collapse the breath / gas
/// clause onto Poisoned-immunity (the only in-engine consequence of
/// "harmful gas"). Uses the new `condition_immunities` lane on `Item`
/// so the install gate in `add_condition` skips Poisoned silently
/// while the necklace is carried.
pub static NECKLACE_OF_ADAPTATION: Item = Item {
    name: "Necklace of Adaptation",
    glyph: 'n',
    condition_immunities: &[crate::conditions::Condition::Poisoned],
    ..Item::DEFAULTS
};

/// Ring of Free Action — passive trinket. The wearer ignores
/// movement-impeding effects: Paralyzed, Restrained, and Grappled
/// can't install while it's carried. 5e RAW: "magic can't reduce your
/// speed or cause you to be paralyzed or restrained"; we model the
/// paralysis / restraint half via condition immunity and surface the
/// Grappled clause too (grappled is mechanically a movement-zero
/// effect, matching the spirit of "free action"). Stacks with item
/// immunities — the install gate honors the union of all carried
/// trinkets.
pub static RING_OF_FREE_ACTION: Item = Item {
    name: "Ring of Free Action",
    glyph: 'r',
    condition_immunities: &[
        crate::conditions::Condition::Paralyzed,
        crate::conditions::Condition::Restrained,
        crate::conditions::Condition::Grappled,
    ],
    ..Item::DEFAULTS
};

/// Pearl of Power — caster-flavored consumable. Bonus action: restore
/// one expended level-1 spell slot to the holder, then the pearl is
/// consumed. 5e RAW: "once per long rest, restore one expended spell
/// slot of level 3 or lower" — we ladder the loot pool through three
/// pearl tiers instead (level-1 / level-2 / level-3 refunds), trading
/// the single-RAW-pearl for three distinct rolls. Drops the long-rest
/// gate in favor of one-shot consumption since the engine doesn't
/// model multi-encounter rest cycles.
pub static PEARL_OF_POWER: Item = Item {
    name: "Pearl of Power",
    glyph: 'q',
    on_use: Some(&crate::actions::item_actions::USE_PEARL_OF_POWER),
    ..Item::DEFAULTS
};

/// Greater Pearl of Power — refunds one expended level-2 spell slot.
/// Sibling to `PEARL_OF_POWER` (level-1 refund); same one-shot envelope
/// but a higher tier. Fires through the shared `PearlOfPowerItem` impl.
pub static GREATER_PEARL_OF_POWER: Item = Item {
    name: "Greater Pearl of Power",
    glyph: 'E',
    on_use: Some(&crate::actions::item_actions::USE_GREATER_PEARL_OF_POWER),
    ..Item::DEFAULTS
};

/// Supreme Pearl of Power — refunds one expended level-3 spell slot.
/// Top of the pearl ladder; matches the RAW pearl's "level 3 or lower"
/// envelope. Same one-shot envelope as the lesser tiers.
pub static SUPREME_PEARL_OF_POWER: Item = Item {
    name: "Supreme Pearl of Power",
    glyph: 'I',
    on_use: Some(&crate::actions::item_actions::USE_SUPREME_PEARL_OF_POWER),
    ..Item::DEFAULTS
};

/// Brooch of Shielding — passive trinket. Grants resistance to force
/// damage while worn. 5e RAW: "you have resistance to force damage,
/// and you are immune to magic missile" — we collapse to the
/// resistance half (the only flavor that fires through the engine's
/// damage pipeline; Magic Missile's force damage gets halved cleanly).
/// Uses the new `damage_resistances` lane on `Item` so the resistance
/// folds through `effective_damage` alongside template and condition
/// sources without code changes at the damage site.
pub static BROOCH_OF_SHIELDING: Item = Item {
    name: "Brooch of Shielding",
    glyph: 'k',
    damage_resistances: &[crate::engine::types::DamageType::Force],
    ..Item::DEFAULTS
};

/// Boots of the Winterlands — passive trinket. Grants resistance to
/// cold damage while worn. 5e RAW: "you have resistance to cold damage
/// and ignore difficult terrain created by ice or snow" — we collapse
/// to the resistance clause (the engine doesn't yet flavor icy terrain
/// as distinct difficult terrain, so the ignore-terrain half would
/// no-op anyway).
pub static BOOTS_OF_THE_WINTERLANDS: Item = Item {
    name: "Boots of the Winterlands",
    glyph: 'W',
    damage_resistances: &[crate::engine::types::DamageType::Cold],
    ..Item::DEFAULTS
};

/// Boots of Speed — bonus action: gain the `Hasted` condition for 10
/// rounds (+2 AC, advantage on DEX saves, doubled walking speed), then
/// the boots are consumed. 5e RAW: action to double speed for up to
/// 10 minutes; we collapse the duration to combat-scale (10 rounds ≈
/// 1 minute) and route through the existing `Hasted` condition for
/// the AC / DEX-save / speed bundle. Pairs with the new
/// `WEAR_BOOTS_OF_SPEED` action. Single-shot consumable — the boots
/// are "exhausted" after one click and removed from inventory.
pub static BOOTS_OF_SPEED: Item = Item {
    name: "Boots of Speed",
    glyph: 'V',
    on_use: Some(&crate::actions::item_actions::WEAR_BOOTS_OF_SPEED),
    ..Item::DEFAULTS
};

/// Periapt of Proof against Poison — passive trinket. Grants the wearer
/// immunity to poison damage AND immunity to the Poisoned condition.
/// 5e RAW: "you are immune to poison damage and the poisoned condition."
/// Strictly stronger than Necklace of Adaptation (which only covers the
/// condition half) — sits as a single rare entry in the loot pool. Uses
/// both the `damage_immunities` lane (zeroes poison damage in
/// `effective_damage`) and the `condition_immunities` lane (blocks the
/// Poisoned install in `add_condition`) so the two halves flow through
/// the same chokepoints that already handle every other source.
pub static PERIAPT_OF_PROOF_AGAINST_POISON: Item = Item {
    name: "Periapt of Proof against Poison",
    glyph: 'y',
    condition_immunities: &[crate::conditions::Condition::Poisoned],
    damage_immunities: &[crate::engine::types::DamageType::Poison],
    ..Item::DEFAULTS
};

/// Ring of Mind Shielding — passive trinket. Grants the wearer immunity
/// to the Charmed condition (mind-control protection) and to psychic
/// damage (mental shielding extends to direct thought-attacks). 5e RAW:
/// "you are immune to magic that allows other creatures to read your
/// thoughts, determine whether you are lying, know your alignment, or
/// know your creature type" — we collapse the divination clauses onto
/// the load-bearing combat clauses (Charmed-immunity for the mental-
/// control half, Psychic-damage immunity for the thought-attack half),
/// since the engine has no divination subsystem. Pairs cleanly with
/// Periapt of Proof against Poison in the rare-trinket tier — same
/// dual-lane shape, different damage type / condition pair.
pub static RING_OF_MIND_SHIELDING: Item = Item {
    name: "Ring of Mind Shielding",
    glyph: 'M',
    condition_immunities: &[crate::conditions::Condition::Charmed],
    damage_immunities: &[crate::engine::types::DamageType::Psychic],
    ..Item::DEFAULTS
};

/// Robe of the Archmagi — premium passive caster trinket. +2 AC and +2
/// to all saves, a strict upgrade on the Cloak of Protection (+1/+1).
/// 5e RAW: also grants advantage on saves vs spells and a spell save DC
/// bump — we collapse those clauses onto the load-bearing flat +2 save
/// bonus since the engine routes most save modifiers through the same
/// `ItemBonuses.save` lane. The +2 AC half is the unarmored-defense
/// equivalent for casters who don't wear heavy armor. Top-of-pool loot
/// — strictly stronger than every other passive trinket, so it sits as
/// a single low-weight entry.
pub static ROBE_OF_THE_ARCHMAGI: Item = Item {
    name: "Robe of the Archmagi",
    glyph: 'R',
    bonuses: ItemBonuses { ac: 2, save: 2, ..ItemBonuses::ZERO },
    ..Item::DEFAULTS
};

/// Ring of Fire Resistance — passive trinket. Grants resistance to fire
/// damage while worn. 5e RAW: "you have resistance to fire damage." Sits
/// in the same "single-typed-resistance ring" tier as Brooch of Shielding
/// (force) and Boots of the Winterlands (cold) — distinct entries per
/// elemental type let the loot table cover a spread of common AoE
/// damage profiles without piling every resistance onto a single
/// overloaded slot.
pub static RING_OF_FIRE_RESISTANCE: Item = Item {
    name: "Ring of Fire Resistance",
    glyph: 'f',
    damage_resistances: &[crate::engine::types::DamageType::Fire],
    ..Item::DEFAULTS
};

/// Ring of Cold Resistance — passive trinket. Grants resistance to cold
/// damage while worn. Mirror of `RING_OF_FIRE_RESISTANCE` for the cold
/// damage type. Sibling to `BOOTS_OF_THE_WINTERLANDS` (also cold
/// resistance), but the ring is the loot slot that doesn't compete with
/// the boots' speed bonus / movement-bonus tier.
pub static RING_OF_COLD_RESISTANCE: Item = Item {
    name: "Ring of Cold Resistance",
    glyph: 'o',
    damage_resistances: &[crate::engine::types::DamageType::Cold],
    ..Item::DEFAULTS
};

/// Ring of Acid Resistance — passive trinket. Grants resistance to acid
/// damage while worn. Slots into the elemental-resistance ring family
/// next to fire / cold / lightning so the loot pool spreads coverage
/// over the four classic burst-damage elements.
pub static RING_OF_ACID_RESISTANCE: Item = Item {
    name: "Ring of Acid Resistance",
    glyph: 'd',
    damage_resistances: &[crate::engine::types::DamageType::Acid],
    ..Item::DEFAULTS
};

/// Ring of Lightning Resistance — passive trinket. Grants resistance to
/// lightning damage while worn. Final entry in the four-element ring
/// family (fire / cold / acid / lightning).
pub static RING_OF_LIGHTNING_RESISTANCE: Item = Item {
    name: "Ring of Lightning Resistance",
    glyph: 'g',
    damage_resistances: &[crate::engine::types::DamageType::Lightning],
    ..Item::DEFAULTS
};

/// Ring of Poison Resistance — passive trinket. Grants resistance to
/// poison damage while worn. Distinct from the Periapt of Proof
/// against Poison (immunity + Poisoned-condition-immune) — this is the
/// resistance-tier counterpart at a different weight in the loot pool.
/// Slots into the typed-resistance ring family alongside fire / cold /
/// acid / lightning.
pub static RING_OF_POISON_RESISTANCE: Item = Item {
    name: "Ring of Poison Resistance",
    glyph: 'j',
    damage_resistances: &[crate::engine::types::DamageType::Poison],
    ..Item::DEFAULTS
};

/// Ring of Radiant Resistance — passive trinket. Grants resistance to
/// radiant damage while worn. Useful against celestial / cleric burst
/// (Spirit Guardians, Sacred Burst, Sunburst). Slots into the typed-
/// resistance ring family alongside the elemental rings.
pub static RING_OF_RADIANT_RESISTANCE: Item = Item {
    name: "Ring of Radiant Resistance",
    glyph: 'u',
    damage_resistances: &[crate::engine::types::DamageType::Radiant],
    ..Item::DEFAULTS
};

/// Ring of Necrotic Resistance — passive trinket. Grants resistance to
/// necrotic damage while worn. Counterpart to the Radiant ring — useful
/// against undead drain attacks and wizard necromancy bursts (Blight,
/// Circle of Death). Slots into the typed-resistance ring family.
pub static RING_OF_NECROTIC_RESISTANCE: Item = Item {
    name: "Ring of Necrotic Resistance",
    glyph: 'e',
    damage_resistances: &[crate::engine::types::DamageType::Necrotic],
    ..Item::DEFAULTS
};

/// Ring of Thunder Resistance — passive trinket. Grants resistance to
/// thunder damage while worn. Useful against Shatter / Thunderwave /
/// Thunder Step bursts. Slots into the typed-resistance ring family
/// alongside the elemental rings.
pub static RING_OF_THUNDER_RESISTANCE: Item = Item {
    name: "Ring of Thunder Resistance",
    glyph: 'v',
    damage_resistances: &[crate::engine::types::DamageType::Thunder],
    ..Item::DEFAULTS
};

/// Ring of Psychic Resistance — passive trinket. Grants resistance to
/// psychic damage while worn. Distinct from the Ring of Mind Shielding
/// (which is full Psychic immunity + Charmed-immunity) — this is the
/// resistance-tier counterpart at a different loot weight. Useful
/// against Mind Sliver / Psychic Scream / Phantasmal-style mental
/// bursts.
pub static RING_OF_PSYCHIC_RESISTANCE: Item = Item {
    name: "Ring of Psychic Resistance",
    glyph: 'x',
    damage_resistances: &[crate::engine::types::DamageType::Psychic],
    ..Item::DEFAULTS
};

/// Scroll of Cone of Cold — single-use 8d8 cold-damage burst. 60-foot
/// cone in RAW; we model as a 6-tile-radius burst centered on the target
/// tile (matching the `CONE_OF_COLD` spell's burst approximation). All
/// actors in the area make a CON save vs DC 15; pass halves, fail takes
/// full. The scroll consumes on use; no spell slot.
pub static SCROLL_OF_CONE_OF_COLD: Item = Item {
    name: "Scroll of Cone of Cold",
    glyph: 'O',
    on_use: Some(&crate::actions::item_actions::READ_CONE_OF_COLD_SCROLL),
    ..Item::DEFAULTS
};

/// Wand of Magic Missiles — single-use 5-dart variant of the Magic
/// Missile spell. Each dart deals 1d4+1 force damage at one enemy in
/// line-of-sight (range 30 tiles), auto-hit / no save. Mirrors
/// `SCROLL_OF_MAGIC_MISSILE` (which fires 3 darts) — the wand is the
/// upgraded loot slot. 5e RAW: the wand has 7 charges and casts at level
/// 1-3; we collapse to a single-shot consumable for the engine's
/// charge-less loot model, sized at the level-2 cast (5 darts) so it
/// lands between the scroll's 3 darts and a level-3 wizard's 5 darts.
pub static WAND_OF_MAGIC_MISSILES: Item = Item {
    name: "Wand of Magic Missiles",
    glyph: 'D',
    on_use: Some(&crate::actions::item_actions::USE_WAND_OF_MAGIC_MISSILES),
    ..Item::DEFAULTS
};

/// Wand of Fireballs — single-use 8d6 fire burst (Action, 60-ft range,
/// 4-tile radius, DEX save vs DC 15 for half). Sits a tier above the
/// `SCROLL_OF_FIREBALL` (6d6) — same shape, bigger payload. The wand
/// is consumed after one click; no charges tracked.
pub static WAND_OF_FIREBALLS: Item = Item {
    name: "Wand of Fireballs",
    glyph: 'F',
    on_use: Some(&crate::actions::item_actions::USE_WAND_OF_FIREBALLS),
    ..Item::DEFAULTS
};

/// Wand of Lightning Bolts — single-use 10d6 lightning burst (Action,
/// 100-ft range, 2-tile radius, DEX save vs DC 15 for half). Sits a tier
/// above `SCROLL_OF_LIGHTNING_BOLT` (8d6) — same shape, bigger payload.
/// Sibling to `WAND_OF_FIREBALLS` for the lightning lane.
pub static WAND_OF_LIGHTNING_BOLTS: Item = Item {
    name: "Wand of Lightning Bolts",
    glyph: 'Z',
    on_use: Some(&crate::actions::item_actions::USE_WAND_OF_LIGHTNING_BOLTS),
    ..Item::DEFAULTS
};

/// Potion of Flying — Action; grants the holder the `Flying` condition
/// for 10 rounds (≈1 minute RAW combat-scaled). Re-uses the existing
/// Flying condition so the +24-tile speed bump and ranged-attack
/// deflection flow through the same accessors a normal Fly cast does.
/// Single-use consumable.
pub static POTION_OF_FLYING: Item = Item {
    name: "Potion of Flying",
    glyph: '^',
    on_use: Some(&crate::actions::item_actions::DRINK_POTION_OF_FLYING),
    ..Item::DEFAULTS
};

/// Potion of Climbing — Bonus Action; grants the holder the
/// `SpiderClimbing` condition for 10 rounds. Cheaper / lesser mobility
/// envelope than Potion of Flying (Action cost, full flight). Re-uses
/// the Spider Climb condition so the +12-tile speed bump flows through
/// the same accessor a normal Spider Climb cast does. Single-use
/// consumable.
pub static POTION_OF_CLIMBING: Item = Item {
    name: "Potion of Climbing",
    glyph: '*',
    on_use: Some(&crate::actions::item_actions::DRINK_POTION_OF_CLIMBING),
    ..Item::DEFAULTS
};

/// Potion of Superior Healing — Action; 8d4+8 self-heal. Top of the
/// healing-potion tier: Healing (2d4+2) → Greater (4d4+4) → Superior
/// (8d4+8). 5e RAW also has a Supreme (10d4+20) tier — we stop at
/// Superior for the loot pool. Single-use consumable.
pub static POTION_OF_SUPERIOR_HEALING: Item = Item {
    name: "Potion of Superior Healing",
    glyph: 'X',
    on_use: Some(&crate::actions::item_actions::DRINK_SUPERIOR_HEALING_POTION),
    ..Item::DEFAULTS
};

/// Potion of Stoneskin — Action; installs `DamageResistant` for 10
/// rounds (halve all incoming damage). Mirrors the Stoneskin spell's
/// envelope (the spell installs the same condition). Single-use
/// consumable.
pub static POTION_OF_STONESKIN: Item = Item {
    name: "Potion of Stoneskin",
    glyph: 'T',
    on_use: Some(&crate::actions::item_actions::DRINK_POTION_OF_STONESKIN),
    ..Item::DEFAULTS
};

/// Wand of Cone of Cold — single-use 10d8 cold CON-save burst (Action,
/// 6-tile radius). Sits a tier above `SCROLL_OF_CONE_OF_COLD` (8d8) —
/// same shape, bigger pool. Top-of-pool burst-wand entry alongside the
/// fire / lightning wand siblings.
pub static WAND_OF_CONE_OF_COLD: Item = Item {
    name: "Wand of Cone of Cold",
    glyph: 'Q',
    on_use: Some(&crate::actions::item_actions::USE_WAND_OF_CONE_OF_COLD),
    ..Item::DEFAULTS
};

/// Scroll of Shatter — single-use 3d8 thunder CON-save burst (Action,
/// 2-tile radius). Fills the thunder lane in the burst-damage scroll
/// family — alongside Fireball (fire), Lightning Bolt (lightning), and
/// Cone of Cold (cold).
pub static SCROLL_OF_SHATTER: Item = Item {
    name: "Scroll of Shatter",
    glyph: 't',
    on_use: Some(&crate::actions::item_actions::READ_SHATTER_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Mass Healing Word — single-use ally-aura heal. Bonus action;
/// heal up to 6 nearest allies (combat-active or dying) within 60 ft
/// (24 tiles) of the reader for 1d4+3 HP each. Fires through the
/// `READ_MASS_HEALING_WORD_SCROLL` action, which mirrors the
/// `MASS_HEALING_WORD` spell's envelope at a fixed +3 caster-mod
/// stand-in. Sits in the loot pool as the multi-target counterpart to
/// the single-target Scroll of Cure Wounds.
pub static SCROLL_OF_MASS_HEALING_WORD: Item = Item {
    name: "Scroll of Mass Healing Word",
    glyph: 'z',
    on_use: Some(&crate::actions::item_actions::READ_MASS_HEALING_WORD_SCROLL),
    ..Item::DEFAULTS
};

/// +1 Weapon — passive trinket. Grants +1 to attack rolls AND +1 to
/// damage rolls while carried. 5e RAW: a magical weapon adds the bonus
/// to both attack and damage with that weapon; we abstract over the
/// weapon-vs-weapon binding (the engine doesn't model weapon ownership
/// beyond "this swing came from this actor") and have the trinket
/// modify every attack the holder makes — weapon swings AND spell
/// attacks alike. Sits in the loot pool as the common "magical weapon"
/// archetype; the +2 and +3 tiers stack the same fields linearly.
pub static WEAPON_PLUS_ONE: Item = Item {
    name: "+1 Weapon",
    glyph: '/',
    bonuses: ItemBonuses {
        attack_bonus: 1,
        damage_bonus: 1,
        ..ItemBonuses::ZERO
    },
    ..Item::DEFAULTS
};

/// +2 Weapon — passive trinket. Premium tier of the magical-weapon
/// loot ladder: +2 attack AND +2 damage on every swing. Single-entry
/// in the loot pool (one notch above the common +1 tier).
pub static WEAPON_PLUS_TWO: Item = Item {
    name: "+2 Weapon",
    glyph: '\\',
    bonuses: ItemBonuses {
        attack_bonus: 2,
        damage_bonus: 2,
        ..ItemBonuses::ZERO
    },
    ..Item::DEFAULTS
};

/// Bracers of Archery — passive trinket. RAW: +2 to damage rolls with
/// longbows / shortbows. The engine doesn't yet split swings by weapon
/// type, so we collapse the gate to "all damage rolls" (a small over-
/// tune: a melee fighter wearing the bracers picks up the bonus too,
/// but the loot tier still slots between Weapon +1 (+1/+1) and Weapon
/// +2 (+2/+2) at a useful niche). The attack-bonus stays 0 — the
/// bracers explicitly don't grant a to-hit bump RAW.
pub static BRACERS_OF_ARCHERY: Item = Item {
    name: "Bracers of Archery",
    glyph: 'Y',
    bonuses: ItemBonuses {
        damage_bonus: 2,
        ..ItemBonuses::ZERO
    },
    ..Item::DEFAULTS
};

/// Ioun Stone of Mastery — passive trinket. RAW (DMG): "your proficiency
/// bonus increases by 1 while you have this stone." We collapse the
/// proficiency-bump clause onto the load-bearing attack lane: +1 attack
/// AND +1 save (the two rolls that proficiency-bonus most consequentially
/// drives). Distinct loot tier from `+1 Weapon` since this also stacks
/// the save bonus rather than the damage bonus — caster-flavored.
pub static IOUN_STONE_OF_MASTERY: Item = Item {
    name: "Ioun Stone of Mastery",
    glyph: 'J',
    bonuses: ItemBonuses {
        attack_bonus: 1,
        save: 1,
        ..ItemBonuses::ZERO
    },
    ..Item::DEFAULTS
};

/// Sentinel Shield — passive trinket. RAW (XGtE): "you have advantage on
/// initiative rolls and Wisdom (Perception) checks." We don't model
/// initiative or perception checks at this granularity, so we collapse
/// the trait onto a defensive AC bump (+1, matching the shield slot) and
/// a +1 save bonus (the "alert" half of the trinket). Same numeric profile
/// as Ring of Protection but a different in-fiction flavor — gives the
/// loot pool another low-tier defensive trinket without sliding in a
/// straight Ring of Protection duplicate.
pub static SENTINEL_SHIELD: Item = Item {
    name: "Sentinel Shield",
    glyph: '#',
    bonuses: ItemBonuses { ac: 1, save: 1, ..ItemBonuses::ZERO },
    ..Item::DEFAULTS
};

/// +3 Weapon — passive trinket. Top tier of the magical-weapon loot
/// ladder: +3 attack AND +3 damage on every swing. Single-entry rare
/// drop sitting one notch above `WEAPON_PLUS_TWO`. The numbers stack
/// linearly with the `attack_bonus_buff` / `damage_bonus_buff` lanes
/// the lower tiers already ride, so no new code paths fire.
pub static WEAPON_PLUS_THREE: Item = Item {
    name: "+3 Weapon",
    glyph: '|',
    bonuses: ItemBonuses {
        attack_bonus: 3,
        damage_bonus: 3,
        ..ItemBonuses::ZERO
    },
    ..Item::DEFAULTS
};

/// Belt of Giant Strength — passive trinket. 5e RAW: sets the wearer's
/// STR score to a fixed value (19 for Hill Giant, 25 for Storm Giant);
/// the engine doesn't model overwriting ability scores, so we collapse
/// the STR-set clause onto the load-bearing combat effects of a STR
/// bump: +2 damage on every swing (STR mod's typical +3 → +5 shift) and
/// +10 max HP (the CON-adjacent vitality the belt represents). Distinct
/// from `GAUNTLETS_OF_OGRE_POWER` (+1 AC / +5 HP) — the belt's damage
/// rider is the offensive niche; the gauntlets sit on the defensive lane.
pub static BELT_OF_GIANT_STRENGTH: Item = Item {
    name: "Belt of Giant Strength",
    glyph: '~',
    bonuses: ItemBonuses {
        damage_bonus: 2,
        max_hp: 10,
        ..ItemBonuses::ZERO
    },
    ..Item::DEFAULTS
};

/// Potion of Supreme Healing — Action; 10d4+20 self-heal. Top of the
/// healing-potion tier: Healing (2d4+2) → Greater (4d4+4) → Superior
/// (8d4+8) → Supreme (10d4+20). Matches 5e RAW. Single-use consumable.
pub static POTION_OF_SUPREME_HEALING: Item = Item {
    name: "Potion of Supreme Healing",
    glyph: '%',
    on_use: Some(&crate::actions::item_actions::DRINK_SUPREME_HEALING_POTION),
    ..Item::DEFAULTS
};

/// Potion of Mage Armor — Action; installs `MageArmored` for 10 rounds
/// (AC floor of 13 + DEX). Single-use consumable. The Mage Armor spell
/// is a level-1 abjuration; the potion bypasses the spell-slot cost so
/// non-casters can dip into the buff. Rejects re-drink while the buff is
/// up so the consumable isn't burned on a no-op timer refresh.
pub static POTION_OF_MAGE_ARMOR: Item = Item {
    name: "Potion of Mage Armor",
    glyph: 'N',
    on_use: Some(&crate::actions::item_actions::DRINK_POTION_OF_MAGE_ARMOR),
    ..Item::DEFAULTS
};

/// Potion of Blur — Action; installs `Blurred` for 10 rounds (attacks
/// against the holder have disadvantage). Single-use consumable. The
/// Blur spell is a level-2 concentration; the potion bypasses
/// concentration so the holder can stack it on top of an existing
/// concentration buff. Sits in the loot pool as a defensive consumable
/// alongside Potion of Invisibility / Potion of Stoneskin.
pub static POTION_OF_BLUR: Item = Item {
    name: "Potion of Blur",
    glyph: '?',
    on_use: Some(&crate::actions::item_actions::DRINK_POTION_OF_BLUR),
    ..Item::DEFAULTS
};

/// Greater Wand of Magic Missiles — 7-dart variant of the Magic Missile
/// spell. Top of the MM loot ladder: Scroll (3 darts) → Wand (5 darts)
/// → Greater Wand (7 darts). Matches the RAW level-4 upcast. Single-use
/// consumable. Fires through the shared `MagicMissileItem` impl.
pub static GREATER_WAND_OF_MAGIC_MISSILES: Item = Item {
    name: "Greater Wand of Magic Missiles",
    glyph: '>',
    on_use: Some(&crate::actions::item_actions::USE_GREATER_WAND_OF_MAGIC_MISSILES),
    ..Item::DEFAULTS
};

/// Scroll of Burning Hands — single-use 3d6 fire DEX-save burst (Action,
/// 2-tile radius). Fills the entry-level fire-burst niche in the scroll
/// family — distinct from the rare Scroll of Fireball (6d6). Mirrors
/// the BURNING_HANDS spell at level 1.
pub static SCROLL_OF_BURNING_HANDS: Item = Item {
    name: "Scroll of Burning Hands",
    glyph: '&',
    on_use: Some(&crate::actions::item_actions::READ_BURNING_HANDS_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Thunderwave — single-use 2d8 thunder CON-save burst (Action,
/// 2-tile radius). Fills the thunder lane at the cheap loot tier
/// alongside the rare Scroll of Shatter (3d8). Mirrors the THUNDERWAVE
/// spell at level 1, minus the push rider (the shared BurstSaveDamageItem
/// helper doesn't fork into push follow-ups).
pub static SCROLL_OF_THUNDERWAVE: Item = Item {
    name: "Scroll of Thunderwave",
    glyph: '@',
    on_use: Some(&crate::actions::item_actions::READ_THUNDERWAVE_SCROLL),
    ..Item::DEFAULTS
};

/// Wand of Web — single-use 4-tile burst, DEX save vs DC 15, fail =
/// Restrained for 10 rounds. 5e RAW: 7 charges casting the Web spell;
/// we collapse to a one-shot fire-and-forget cast — no concentration,
/// no charges tracked, no allies caught in the strands (the wand uses
/// the player-friendly enemy-burst lane like every other harmful
/// consumable). Fills the Restrained-installer niche in the loot pool
/// next to the burst-damage scrolls / wands.
pub static WAND_OF_WEB: Item = Item {
    name: "Wand of Web",
    glyph: '$',
    on_use: Some(&crate::actions::item_actions::USE_WAND_OF_WEB),
    ..Item::DEFAULTS
};

/// Pipes of Haunting — single-use 4-tile burst, WIS save vs DC 13,
/// fail = Frightened for 10 rounds. 5e RAW: 30-ft cone fear-burst with
/// 3 charges; we collapse to a one-shot envelope. Lower DC (13 vs the
/// usual 15) reflects the "low-tier mood music" flavor — pairs with the
/// rarer Wand of Fear (also Frightened, DC 15, single-target) so the
/// loot pool covers the Frightened lane at two tiers.
pub static PIPES_OF_HAUNTING: Item = Item {
    name: "Pipes of Haunting",
    glyph: '(',
    on_use: Some(&crate::actions::item_actions::PLAY_PIPES_OF_HAUNTING),
    ..Item::DEFAULTS
};

/// Wand of Paralysis — single-use single-target, CON save vs DC 15,
/// fail = Paralyzed for 10 rounds. 5e RAW: 7 charges firing a line of
/// paralysis at one creature; we collapse to a one-shot beam — no
/// charges tracked. Paralyzed is one of the engine's hardest CC
/// envelopes (zero movement, action economy blocked, auto-fail
/// STR/DEX saves, melee crits land automatically), so the consumable
/// sits in the rare half of the loot pool.
pub static WAND_OF_PARALYSIS: Item = Item {
    name: "Wand of Paralysis",
    glyph: ')',
    on_use: Some(&crate::actions::item_actions::USE_WAND_OF_PARALYSIS),
    ..Item::DEFAULTS
};

/// Wand of Fear — single-use single-target, WIS save vs DC 15, fail =
/// Frightened for 10 rounds. 5e RAW: 7 charges casting Fear (a 30-ft
/// cone) at level 3; we collapse to a single-target single-use cast.
/// Distinct from Pipes of Haunting (same condition, wider burst, lower
/// DC) — the wand is the "hard single-target fear" niche, the pipes
/// cover the "soft area fear" niche.
pub static WAND_OF_FEAR: Item = Item {
    name: "Wand of Fear",
    glyph: '[',
    on_use: Some(&crate::actions::item_actions::USE_WAND_OF_FEAR),
    ..Item::DEFAULTS
};

/// Scroll of Hold Person — single-use single-target, WIS save vs DC 13,
/// fail = Paralyzed for 10 rounds. 5e RAW: level-2 enchantment with
/// concentration / re-save each turn; the scroll collapses to the
/// fixed 10-round paralysis envelope every other CC consumable rides.
/// Entry-level CC scroll alongside Pipes of Haunting (DC 13 burst
/// Frightened) — the scroll is the single-target hard-CC niche at the
/// cheap tier.
pub static SCROLL_OF_HOLD_PERSON: Item = Item {
    name: "Scroll of Hold Person",
    glyph: ']',
    on_use: Some(&crate::actions::item_actions::READ_HOLD_PERSON_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Hold Monster — single-use single-target, WIS save vs DC 15,
/// fail = Paralyzed for 10 rounds. Sits a tier above Scroll of Hold
/// Person (DC 13, 24-tile reach) — same shape, harder DC, longer reach
/// (90 ft RAW).
pub static SCROLL_OF_HOLD_MONSTER: Item = Item {
    name: "Scroll of Hold Monster",
    glyph: '{',
    on_use: Some(&crate::actions::item_actions::READ_HOLD_MONSTER_SCROLL),
    ..Item::DEFAULTS
};

/// Wand of Confusion — single-use 4-tile burst, WIS save vs DC 15,
/// fail = Confused for 10 rounds (disadvantage on attacks AND no
/// reactions). 5e RAW: level-4 enchantment, 90-ft range / 10-ft cube,
/// concentration; we collapse to a single-shot fire-and-forget cast.
/// Top-of-pool burst CC alongside Wand of Paralysis — the confusion
/// wand trades single-target lockdown for a wider soft-CC blanket.
pub static WAND_OF_CONFUSION: Item = Item {
    name: "Wand of Confusion",
    glyph: '}',
    on_use: Some(&crate::actions::item_actions::USE_WAND_OF_CONFUSION),
    ..Item::DEFAULTS
};

/// Scroll of Hypnotic Pattern — single-use 4-tile burst, WIS save vs
/// DC 14, fail = Incapacitated for 10 rounds. 5e RAW: level-3 illusion,
/// 120-ft range / 30-ft cube, concentration; we collapse to a single-
/// shot fire-and-forget cast. Mid-tier burst CC between Pipes of
/// Haunting (DC 13 Frightened) and Wand of Confusion (DC 15 Confused).
pub static SCROLL_OF_HYPNOTIC_PATTERN: Item = Item {
    name: "Scroll of Hypnotic Pattern",
    glyph: '`',
    on_use: Some(&crate::actions::item_actions::READ_HYPNOTIC_PATTERN_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Vitriolic Sphere — single-use 10d4 acid DEX-save burst,
/// 4-tile radius. Fills the acid lane in the burst-damage scroll family
/// alongside Fireball (fire), Lightning Bolt (lightning), Cone of Cold
/// (cold), and Shatter (thunder).
pub static SCROLL_OF_VITRIOLIC_SPHERE: Item = Item {
    name: "Scroll of Vitriolic Sphere",
    glyph: ':',
    on_use: Some(&crate::actions::item_actions::READ_VITRIOLIC_SPHERE_SCROLL),
    ..Item::DEFAULTS
};

/// Archmage Pearl of Power — bonus action; restore one expended level-4
/// spell slot. Top of the pearl ladder above Supreme Pearl of Power
/// (level-3 refund). 5e RAW pearls cap at level-3 slots; we extend the
/// ladder to cover the level-4 slot tier as the rarest-tier caster
/// consumable.
pub static ARCHMAGE_PEARL_OF_POWER: Item = Item {
    name: "Archmage Pearl of Power",
    glyph: ';',
    on_use: Some(&crate::actions::item_actions::USE_ARCHMAGE_PEARL_OF_POWER),
    ..Item::DEFAULTS
};

/// Potion of Sanctuary — Bonus Action; installs `Sanctuary` for 10
/// rounds. 5e RAW: the spell is level-1 abjuration, bonus action, on a
/// willing target; the potion collapses to a self-only envelope. The
/// buff routes hostile actions against the drinker through a WIS save
/// vs the source's DC (silently no-ops on fail) and drops the moment
/// the drinker themselves attacks or casts a damaging spell.
pub static POTION_OF_SANCTUARY: Item = Item {
    name: "Potion of Sanctuary",
    glyph: ',',
    on_use: Some(&crate::actions::item_actions::DRINK_POTION_OF_SANCTUARY),
    ..Item::DEFAULTS
};

/// Wand of Cure Wounds — Action; touch (1-tile) ally heal for 3d8+3.
/// Sits a tier above the Scroll of Cure Wounds (2d8+2) — same shape,
/// bigger pool. 5e RAW: 7 charges casting Cure Wounds at level 1-3;
/// we collapse to a single 3d8 cast for the engine's charge-less loot
/// model.
pub static WAND_OF_CURE_WOUNDS: Item = Item {
    name: "Wand of Cure Wounds",
    glyph: '"',
    on_use: Some(&crate::actions::item_actions::USE_WAND_OF_CURE_WOUNDS),
    ..Item::DEFAULTS
};

/// Scroll of Healing Word — Bonus Action; ranged ally heal for 1d4+3
/// at 24-tile reach. 5e RAW: level-1 evocation, bonus action, 60-ft
/// range. Pairs with the touch-range Scroll of Cure Wounds — the
/// healing-word scroll trades payload for reach and action economy.
pub static SCROLL_OF_HEALING_WORD: Item = Item {
    name: "Scroll of Healing Word",
    glyph: '\'',
    on_use: Some(&crate::actions::item_actions::READ_HEALING_WORD_SCROLL),
    ..Item::DEFAULTS
};

/// Potion of Growth — Action; installs `Enlarged` for 10 rounds
/// (+1d4 weapon damage rider, size bump). 5e RAW: 1d4-hour duration;
/// we collapse to the combat-scale 10-round timer every other buff
/// consumable rides. Sibling to Belt of Giant Strength on the
/// offensive bruiser lane — the belt is a passive +2 damage / +10 HP,
/// the potion is a single-shot +1d4 damage rider on hits.
pub static POTION_OF_GROWTH: Item = Item {
    name: "Potion of Growth",
    glyph: '<',
    on_use: Some(&crate::actions::item_actions::DRINK_POTION_OF_GROWTH),
    ..Item::DEFAULTS
};

/// Wand of Greater Healing — Action; touch ally heal for 4d8+4. Top
/// tier of the single-target ally-heal ladder: Scroll of Cure Wounds
/// (2d8+2) → Wand of Cure Wounds (3d8+3) → Wand of Greater Healing
/// (4d8+4).
pub static WAND_OF_GREATER_HEALING: Item = Item {
    name: "Wand of Greater Healing",
    glyph: 'K',
    on_use: Some(&crate::actions::item_actions::USE_WAND_OF_GREATER_HEALING),
    ..Item::DEFAULTS
};

/// Pool of items that can be dropped as random loot. Order is irrelevant;
/// the encounter picks uniformly. Add new specials here to put them in
/// rotation without touching call sites. Some entries appear multiple
/// times to weight the drop table toward more common items.
pub static LOOT_POOL: &[&Item] = &[
    &RING_OF_PROTECTION,
    &BOOTS_OF_STRIDING,
    &CLOAK_OF_RESISTANCE,
    &CLOAK_OF_PROTECTION,
    &AMULET_OF_HEALTH,
    &HEADBAND_OF_INSIGHT,
    &BRACERS_OF_DEFENSE,
    // Healing potions sit at higher weight — they're consumables and
    // expected to outpace passive trinkets across a dungeon.
    &POTION_OF_HEALING,
    &POTION_OF_HEALING,
    &POTION_OF_HEALING,
    &POTION_OF_GREATER_HEALING,
    &ANTITOXIN,
    &POTION_OF_SPEED,
    &POTION_OF_HEROISM,
    &POTION_OF_INVISIBILITY,
    &SCROLL_OF_FIREBALL,
    &SCROLL_OF_MAGIC_MISSILE,
    &SCROLL_OF_LIGHTNING_BOLT,
    // Single-target healing scroll — slots into the loot pool between
    // the self-only Potion of Healing and the bonus-action Greater
    // Healing variant. Touch-range, so it benefits front-line allies
    // (the rogue / fighter / paladin) without needing a caster.
    &SCROLL_OF_CURE_WOUNDS,
    &PERIAPT_OF_WOUND_CLOSURE,
    &GAUNTLETS_OF_OGRE_POWER,
    // Premium passive trinkets — same low odds as Cloak of Protection
    // / Ring of Protection; the immunity rings are stronger than the
    // generic save / AC bumps so they sit on the rare half of the
    // pool by virtue of single-entry weighting.
    &STONE_OF_GOOD_LUCK,
    &NECKLACE_OF_ADAPTATION,
    &RING_OF_FREE_ACTION,
    // Caster-side consumables — Pearl restores a slot, Boots grant
    // Hasted. Slot in at the same weight as the offensive scrolls so
    // arcane casters have a roughly even shot at a defensive utility.
    &PEARL_OF_POWER,
    &BOOTS_OF_SPEED,
    // Typed-resistance trinkets — same weight as the AC / save
    // trinkets, but the resistance lane fires through the damage
    // pipeline rather than the AC / save lanes.
    &BROOCH_OF_SHIELDING,
    &BOOTS_OF_THE_WINTERLANDS,
    // Typed-immunity trinkets — single low-weight entries since
    // immunity is strictly stronger than resistance and these double
    // up as condition-immunity sources too. Both ride the new
    // `damage_immunities` lane.
    &PERIAPT_OF_PROOF_AGAINST_POISON,
    &RING_OF_MIND_SHIELDING,
    // Premium caster trinket — top of the passive tier, single entry
    // by design. Strictly dominates Cloak of Protection (+1/+1) and
    // Ring of Protection (+1/+1).
    &ROBE_OF_THE_ARCHMAGI,
    // Elemental-resistance ring family — single low-weight entry per
    // element so the loot table covers the four classic burst-damage
    // types (fire / cold / acid / lightning) without over-skewing the
    // pool toward typed-resistance loot.
    &RING_OF_FIRE_RESISTANCE,
    &RING_OF_COLD_RESISTANCE,
    &RING_OF_ACID_RESISTANCE,
    &RING_OF_LIGHTNING_RESISTANCE,
    // Secondary-resistance ring family — covers the remaining damage
    // types the engine actually fires (poison / radiant / necrotic /
    // thunder / psychic). Same single-entry weighting as the elemental
    // four so the loot pool spreads coverage across every burst type
    // without piling weight onto any single resistance source.
    &RING_OF_POISON_RESISTANCE,
    &RING_OF_RADIANT_RESISTANCE,
    &RING_OF_NECROTIC_RESISTANCE,
    &RING_OF_THUNDER_RESISTANCE,
    &RING_OF_PSYCHIC_RESISTANCE,
    // Single-use AoE scrolls / wands — same weight as the Fireball /
    // Lightning Bolt scrolls so casters have a roughly even shot at a
    // big burst regardless of element. Cone of Cold's 8d8 cold tier sits
    // above the 6d6 Fireball / 8d6 Lightning Bolt tier; Wand of Magic
    // Missiles' 5-dart payload sits above the 3-dart scroll tier.
    &SCROLL_OF_CONE_OF_COLD,
    &WAND_OF_MAGIC_MISSILES,
    // Wand of Fireballs / Lightning Bolts sit one tier above their
    // scroll counterparts: 8d6 fire vs 6d6, 10d6 lightning vs 8d6.
    &WAND_OF_FIREBALLS,
    &WAND_OF_LIGHTNING_BOLTS,
    // Mobility potions — Flying is the premium full-flight envelope,
    // Climbing is the cheaper bonus-action variant. Single entries each
    // since mobility buffs are situationally strong (kiting / chasing).
    &POTION_OF_FLYING,
    &POTION_OF_CLIMBING,
    // Superior Healing — top-tier consumable heal. Single entry weights
    // it below the regular Potion of Healing (3 entries) — it's a
    // premium drop.
    &POTION_OF_SUPERIOR_HEALING,
    // Stoneskin — premium defensive consumable; halves all damage for
    // 10 rounds. Single entry — same tier as the premium passive
    // trinkets.
    &POTION_OF_STONESKIN,
    // Wand of Cone of Cold rounds out the burst-wand trio (fire /
    // lightning / cold), all at the "scroll + 1 tier" pool size.
    &WAND_OF_CONE_OF_COLD,
    // Scroll of Shatter fills the thunder lane in the scroll family
    // alongside fire / lightning / cold.
    &SCROLL_OF_SHATTER,
    // Pearl-of-Power ladder — three tiers of spell-slot refunds. Single
    // entries each since slot-refund consumables are situationally
    // strong (the Supreme Pearl refunds a level-3 slot worth far more
    // than the base Pearl). Sits one tier above Boots of Speed in the
    // caster-consumable lane.
    &GREATER_PEARL_OF_POWER,
    &SUPREME_PEARL_OF_POWER,
    // Mass Healing Word scroll — bonus-action ally aura heal. Single
    // entry; complements the single-target Cure Wounds scroll for
    // multi-ally emergency healing.
    &SCROLL_OF_MASS_HEALING_WORD,
    // Magical weapon ladder — +1 sits at common weight (mirroring
    // Cloak of Resistance / Ring of Protection); +2 is single-entry
    // premium. Both grant +N attack AND +N damage on every swing so
    // the loot tier slots cleanly between trinkets (defensive) and
    // burst scrolls (offensive).
    &WEAPON_PLUS_ONE,
    &WEAPON_PLUS_ONE,
    &WEAPON_PLUS_TWO,
    // Bracers of Archery — +2 damage trinket. Same weight as the
    // single-element resistance rings; sits as an offensive-niche
    // trinket alongside the defensive AC / save bumps.
    &BRACERS_OF_ARCHERY,
    // Ioun Stone of Mastery — caster-flavored +1/+1 (attack/save) trinket.
    // Sits alongside Stone of Good Luck (+1/+1 AC/save) and the Cloak of
    // Protection (+1/+1 AC/save) as a third "+1 to two stats" passive.
    &IOUN_STONE_OF_MASTERY,
    // Sentinel Shield — low-tier defensive trinket. Same weight as the
    // generic Ring of Protection / Cloak of Protection siblings.
    &SENTINEL_SHIELD,
    // +3 Weapon — top tier of the magical-weapon ladder. Single-entry
    // rare drop, paired with the existing +1 (common, weight 2) and
    // +2 (single entry) tiers.
    &WEAPON_PLUS_THREE,
    // Belt of Giant Strength — offensive bruiser trinket: +damage and
    // +max-HP. Single low-weight entry alongside Gauntlets of Ogre
    // Power.
    &BELT_OF_GIANT_STRENGTH,
    // Supreme Healing — rarest tier of the healing-potion ladder.
    // Single low-weight entry above Superior Healing (also single).
    &POTION_OF_SUPREME_HEALING,
    // Caster-flavored buff potions. Same weight as the Stoneskin /
    // Invisibility tier — defensive consumables for low-AC casters.
    &POTION_OF_MAGE_ARMOR,
    &POTION_OF_BLUR,
    // Greater Wand of Magic Missiles — top tier of the MM ladder
    // (3-dart scroll → 5-dart wand → 7-dart greater wand).
    &GREATER_WAND_OF_MAGIC_MISSILES,
    // Entry-level burst scrolls — fire / thunder lane at the common
    // weight tier (one notch below the Fireball / Lightning Bolt
    // scrolls in damage payload).
    &SCROLL_OF_BURNING_HANDS,
    &SCROLL_OF_THUNDERWAVE,
    // Crowd-control consumables — burst save-or-condition wands fill
    // the CC niche alongside the burst-damage scrolls / wands. Single
    // entry per item; the Restrained / Frightened / Paralyzed lanes
    // are all situationally strong (a paralyzed boss is effectively
    // free damage for the rest of the party), so the loot pool keeps
    // them rare.
    &WAND_OF_WEB,
    &PIPES_OF_HAUNTING,
    &WAND_OF_PARALYSIS,
    &WAND_OF_FEAR,
    // Hold Person / Monster scrolls — single-target Paralyzed at the
    // cheap (DC 13, 24-tile) and rare (DC 15, 36-tile) tiers. Sibling
    // to the Wand of Paralysis (DC 15, 24-tile) — the scroll variants
    // cover the entry-level lockdown niche and the long-reach niche
    // respectively.
    &SCROLL_OF_HOLD_PERSON,
    &SCROLL_OF_HOLD_MONSTER,
    // Burst CC scrolls — Hypnotic Pattern (Incapacitated DC 14) and
    // Wand of Confusion (Confused DC 15). Fill the mid- and top-tier
    // burst CC slots between Pipes of Haunting (Frightened DC 13) and
    // the single-target Wand of Paralysis.
    &SCROLL_OF_HYPNOTIC_PATTERN,
    &WAND_OF_CONFUSION,
    // Vitriolic Sphere scroll — acid lane in the burst-damage scroll
    // family, sized between Fireball (6d6) and Cone of Cold (8d8).
    &SCROLL_OF_VITRIOLIC_SPHERE,
    // Archmage Pearl of Power — top of the pearl ladder, single entry
    // since level-4 slot refunds are the rarest tier.
    &ARCHMAGE_PEARL_OF_POWER,
    // Potion of Sanctuary — defensive consumable; routes hostile
    // actions through a WIS save until the holder swings back. Single
    // entry alongside the other defensive potions (Mage Armor, Blur).
    &POTION_OF_SANCTUARY,
    // Cure Wounds wand + Healing Word scroll — round out the single-
    // target ally-heal slot. Wand sits a tier above the scroll
    // (3d8+3 vs 2d8+2 touch); the Healing Word scroll covers the
    // long-reach kite-heal niche at BA cost.
    &WAND_OF_CURE_WOUNDS,
    &SCROLL_OF_HEALING_WORD,
    // Potion of Growth — Enlarged buff consumable on the offensive
    // bruiser lane. Single low-weight entry alongside the other
    // size-mod trinkets (Belt of Giant Strength).
    &POTION_OF_GROWTH,
    // Wand of Greater Healing — top tier of the single-target ally
    // heal ladder (2d8+2 scroll → 3d8+3 wand → 4d8+4 greater wand).
    &WAND_OF_GREATER_HEALING,
];
