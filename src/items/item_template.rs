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
    /// Conditions the wearer has installed (Permanent) for as long as the
    /// item is carried. Used by passive-buff trinkets like Slippers of
    /// Spider Climbing (SpiderClimbing) and Winged Boots (Flying) so the
    /// in-fiction effect flows through the same condition map that already
    /// drives `speed()`, `compute_attack_mode`, etc. Hooked at three
    /// places: `pickup_item` installs each entry on grab, `remove_item_by_name`
    /// strips entries that no remaining carried item still grants, and
    /// `long_rest` re-installs them after the conditions clear. A
    /// `Dispel Magic` or other in-combat strip will leave the actor
    /// without the buff until the next long rest (or until the player
    /// re-picks-up the item) — acceptable tradeoff for the simpler
    /// "install on the inventory event" model. Empty for items without a
    /// persistent condition (the default for most loot).
    pub passive_conditions: &'static [crate::conditions::Condition],
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
        passive_conditions: &[],
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

/// Potion of Longstrider — Bonus-Action consumable that installs
/// `Longstriding` (+10 ft speed) for 100 rounds. Mirrors the
/// Longstrider spell's effect for non-casters; cheap-tier mobility
/// consumable alongside Potion of Climbing.
pub static POTION_OF_LONGSTRIDER: Item = Item {
    name: "Potion of Longstrider",
    glyph: '>',
    on_use: Some(&crate::actions::item_actions::DRINK_POTION_OF_LONGSTRIDER),
    ..Item::DEFAULTS
};

/// Scroll of Bless — Action; install `Blessed` for 10 rounds on a single
/// ally (+1d4 to attack rolls and saving throws). Sits in the loot pool
/// as the ally-buff counterpart to the harmful single-target CC scrolls.
pub static SCROLL_OF_BLESS: Item = Item {
    name: "Scroll of Bless",
    glyph: 'B',
    on_use: Some(&crate::actions::item_actions::READ_BLESS_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Shield of Faith — Action; install `ShieldOfFaith` for 10
/// rounds (+2 AC) on a single ally. Pairs with Scroll of Bless on the
/// ally-buff lane — the latter buffs attack rolls / saves, the former
/// boosts AC.
pub static SCROLL_OF_SHIELD_OF_FAITH: Item = Item {
    name: "Scroll of Shield of Faith",
    glyph: 'F',
    on_use: Some(&crate::actions::item_actions::READ_SHIELD_OF_FAITH_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Blindness — Action; single-target, CON save vs DC 13, fail
/// = Blinded for 10 rounds. Fills the single-target Blinded niche
/// alongside Wand of Paralysis (Paralyzed) and Wand of Fear (Frightened).
pub static SCROLL_OF_BLINDNESS: Item = Item {
    name: "Scroll of Blindness",
    glyph: '!',
    on_use: Some(&crate::actions::item_actions::READ_BLINDNESS_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Bane — Action; 4-tile burst, CHA save vs DC 13, fail =
/// Baned for 10 rounds. Mirror of Scroll of Bless on the debuff lane —
/// enemies caught in the burst eat -1d4 to attack rolls and saves.
pub static SCROLL_OF_BANE: Item = Item {
    name: "Scroll of Bane",
    glyph: 'b',
    on_use: Some(&crate::actions::item_actions::READ_BANE_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Faerie Fire — Action; 4-tile burst, DEX save vs DC 13, fail
/// = Outlined for 10 rounds (attacks against them have advantage, can't
/// benefit from Hidden / Invisible). Cheap pre-burst setup for the
/// martial-heavy party.
pub static SCROLL_OF_FAERIE_FIRE: Item = Item {
    name: "Scroll of Faerie Fire",
    glyph: 'i',
    on_use: Some(&crate::actions::item_actions::READ_FAERIE_FIRE_SCROLL),
    ..Item::DEFAULTS
};

/// Wand of Polymorph — Action; single-target, WIS save vs DC 15, fail =
/// Polymorphed for 10 rounds. Top-of-pool single-target CC consumable —
/// Polymorphed shuts down the target's entire spellcasting toolkit.
pub static WAND_OF_POLYMORPH: Item = Item {
    name: "Wand of Polymorph",
    glyph: 'p',
    on_use: Some(&crate::actions::item_actions::USE_WAND_OF_POLYMORPH),
    ..Item::DEFAULTS
};

/// Potion of Barkskin — Bonus Action; installs `Barkskinned` for 10
/// rounds (AC floor of 16). Cheap defensive consumable for low-AC
/// casters; pairs with Potion of Mage Armor (AC 13 + DEX floor) and
/// Potion of Blur (disadvantage on attackers) in the defensive consumable
/// trio.
pub static POTION_OF_BARKSKIN: Item = Item {
    name: "Potion of Barkskin",
    glyph: '+',
    on_use: Some(&crate::actions::item_actions::DRINK_POTION_OF_BARKSKIN),
    ..Item::DEFAULTS
};

/// Potion of Fire Resistance — Action; installs `DamageResistant` for 10
/// rounds. Resists everything in the engine model (the `DamageResistant`
/// condition isn't typed); the loot pool keeps the flavored name to give
/// the player a tactical "elemental shield" feel without proliferating
/// typed-resistance condition variants. Sibling to Potion of Cold
/// Resistance.
pub static POTION_OF_FIRE_RESISTANCE: Item = Item {
    name: "Potion of Fire Resistance",
    glyph: 'F',
    on_use: Some(&crate::actions::item_actions::DRINK_POTION_OF_FIRE_RESISTANCE),
    ..Item::DEFAULTS
};

/// Potion of Cold Resistance — Action; installs `DamageResistant` for 10
/// rounds. Mirror of Potion of Fire Resistance — same envelope, distinct
/// flavor.
pub static POTION_OF_COLD_RESISTANCE: Item = Item {
    name: "Potion of Cold Resistance",
    glyph: 'C',
    on_use: Some(&crate::actions::item_actions::DRINK_POTION_OF_COLD_RESISTANCE),
    ..Item::DEFAULTS
};

/// Potion of Hill Giant Strength — Action; installs `Enlarged` for 10
/// rounds (+1d4 weapon damage rider, size bump). Sibling to Potion of
/// Growth — same condition envelope, distinct in-fiction trigger so the
/// loot pool covers the offensive bruiser consumable lane at two rolls.
pub static POTION_OF_HILL_GIANT_STRENGTH: Item = Item {
    name: "Potion of Hill Giant Strength",
    glyph: 'G',
    on_use: Some(&crate::actions::item_actions::DRINK_POTION_OF_HILL_GIANT_STRENGTH),
    ..Item::DEFAULTS
};

/// Slippers of Spider Climbing — passive trinket that grants the wearer
/// a climbing speed equal to their walking speed (modeled via the
/// `SpiderClimbing` condition's flat +30 ft / +12 tile speed bump). 5e
/// RAW: "you have a climbing speed equal to your walking speed; you can
/// move up, down, and across vertical surfaces and along ceilings." The
/// engine doesn't model vertical terrain — the load-bearing combat clause
/// is the speed bonus, which flows through `condition_speed_bonus`
/// alongside Longstrider / Fly. Sibling to Potion of Climbing (same
/// condition, but the potion is a single-use consumable, while the
/// slippers are passive-on-while-worn).
pub static SLIPPERS_OF_SPIDER_CLIMBING: Item = Item {
    name: "Slippers of Spider Climbing",
    glyph: '_',
    passive_conditions: &[crate::conditions::Condition::SpiderClimbing],
    ..Item::DEFAULTS
};

/// Winged Boots — passive trinket that grants the wearer a flying speed
/// equal to their walking speed (modeled via the `Flying` condition's +60
/// ft / +24 tile speed bump plus the ranged-attacker disadvantage rider).
/// 5e RAW: "while you wear these boots, you have a flying speed equal to
/// your walking speed... a total of 4 hours of flying time, split however
/// you like; recharges 2 hours at dawn." The engine doesn't model fuel
/// reserves — the boots are passive-on-while-worn. Sibling to Potion of
/// Flying (same condition, consumable variant); the boots are the rare-
/// tier permanent counterpart, with the AC bump and ranged-deflection
/// rolling through the same condition the spell installs.
pub static WINGED_BOOTS: Item = Item {
    name: "Winged Boots",
    glyph: 'w',
    passive_conditions: &[crate::conditions::Condition::Flying],
    ..Item::DEFAULTS
};

/// Boots of the Forest — passive trinket. Grants the wearer Longstriding
/// (+10 ft / +4 tile speed bump) while worn. 5e flavor mimic of the
/// Longstrider spell's effect routed through the same condition (joins
/// `condition_speed_bonus` for the additive stack with Boots of Striding's
/// flat ItemBonuses.speed). Sibling to Potion of Longstrider (consumable)
/// — the boots cover the always-on speed-bump niche.
pub static BOOTS_OF_THE_FOREST: Item = Item {
    name: "Boots of the Forest",
    glyph: '~',
    passive_conditions: &[crate::conditions::Condition::Longstriding],
    ..Item::DEFAULTS
};

/// Cloak of Etherealness — passive trinket. Grants the wearer
/// `DamageResistant` (halve all incoming damage) while worn. 5e RAW: the
/// cloak's Etherealness ability lets the wearer enter the Ethereal Plane
/// at will; we collapse the "shift planes to dodge damage" envelope to a
/// flat damage-halve buff that flows through the existing condition lane.
/// Top-of-pool defensive trinket — strictly stronger than the typed-
/// resistance rings.
pub static CLOAK_OF_ETHEREALNESS: Item = Item {
    name: "Cloak of Etherealness",
    glyph: '$',
    passive_conditions: &[crate::conditions::Condition::DamageResistant],
    ..Item::DEFAULTS
};

/// Amulet of the Vigilant — passive trinket. Grants the wearer the
/// Barbarian-style `DangerSense` (always-on advantage on DEX saves while
/// not Blinded / Incapacitated / Deafened). 5e RAW flavor: a stylized
/// amulet that hums warm when danger approaches; we abstract the
/// "preternatural sixth sense" to the Barbarian feature's mechanical
/// envelope. Distinct from the typed-resistance rings — the amulet
/// boosts DEX saves rather than halving damage, sliding cleanly under the
/// "AoE survival" niche for low-DEX casters.
pub static AMULET_OF_THE_VIGILANT: Item = Item {
    name: "Amulet of the Vigilant",
    glyph: 'V',
    passive_conditions: &[crate::conditions::Condition::DangerSense],
    ..Item::DEFAULTS
};

/// Cloak of Displacement — passive trinket (DMG, rare). Light bends around
/// the wearer so attackers see a phantom image a half-step off true: the
/// `Displaced` condition flickers on at install time and self-restores
/// at the start of every turn (so a hit that strips it mid-round comes
/// back next turn). Attackers eat disadvantage on the first swing each
/// round. 5e RAW: "while you wear this cloak, it projects an illusion
/// that makes you appear to be standing in a place near your actual
/// location" — the load-bearing combat clause is the disadvantage rider,
/// which flows through the existing `Displaced` condition install lane.
pub static CLOAK_OF_DISPLACEMENT: Item = Item {
    name: "Cloak of Displacement",
    glyph: 'd',
    passive_conditions: &[crate::conditions::Condition::Displaced],
    ..Item::DEFAULTS
};

/// Scarab of Protection — passive trinket (DMG, legendary). A beetle-
/// shaped amulet that wards the wearer against magical compulsion and
/// fear: dynamic immunity to `Charmed` and `Frightened` while carried,
/// plus a small flat +1 save bonus (the RAW "advantage on saves vs
/// spells" clause collapsed onto the load-bearing save lane, since the
/// engine doesn't have a per-school advantage hook). Pairs with the
/// Necklace of Adaptation (Poisoned-immune) and the Ring of Free Action
/// (movement-condition-immune) in the condition-immunity trinket family.
pub static SCARAB_OF_PROTECTION: Item = Item {
    name: "Scarab of Protection",
    glyph: 'S',
    bonuses: ItemBonuses { save: 1, ..ItemBonuses::ZERO },
    condition_immunities: &[
        crate::conditions::Condition::Charmed,
        crate::conditions::Condition::Frightened,
    ],
    ..Item::DEFAULTS
};

/// Ring of Heroism — passive trinket. Grants the wearer the `Heroic`
/// condition while worn: dynamic immunity to `Frightened` (the load-
/// bearing combat clause of the Heroism spell) plus the temp-HP rider
/// the condition carries. Mirrors the consumable Potion of Heroism on
/// the passive-trinket lane — the ring is always-on, the potion is a
/// one-shot 10-round buff. Sits in the rare half of the loot pool
/// alongside the other condition-installer trinkets (Amulet of the
/// Vigilant / Slippers of Spider Climbing).
pub static RING_OF_HEROISM: Item = Item {
    name: "Ring of Heroism",
    glyph: 'H',
    passive_conditions: &[crate::conditions::Condition::Heroic],
    ..Item::DEFAULTS
};

/// Wand of Sleep — single-use consumable. Single-target WIS save vs DC 13;
/// fail = `Asleep` for 10 rounds. 5e RAW: the Sleep spell is HP-pool based
/// (5d8 HP of creatures fall asleep, lowest first); we collapse to a
/// per-target save envelope every other CC consumable rides. Fills the
/// entry-level lockdown niche alongside Scroll of Hold Person (Paralyzed
/// DC 13) — Sleep wakes on damage (the engine strips `Asleep` on any non-
/// zero hit through the existing wake-on-damage hook), so it pairs with a
/// martial follow-up cleanly.
pub static WAND_OF_SLEEP: Item = Item {
    name: "Wand of Sleep",
    glyph: 'z',
    on_use: Some(&crate::actions::item_actions::USE_WAND_OF_SLEEP),
    ..Item::DEFAULTS
};

/// Scroll of Slow — single-use 4-tile burst, WIS save vs DC 13, fail =
/// `Slowed` for 10 rounds (halved speed, -2 AC, -2 DEX saves). 5e RAW:
/// level-3 transmutation, WIS save, concentration; the scroll collapses
/// to the standard fixed-duration burst envelope and drops the
/// concentration gate. Mirrors Scroll of Bane / Faerie Fire on the burst
/// debuff lane — Slowed is the AC/movement counterpart to Bane's roll
/// penalties.
pub static SCROLL_OF_SLOW: Item = Item {
    name: "Scroll of Slow",
    glyph: 'l',
    on_use: Some(&crate::actions::item_actions::READ_SLOW_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Stinking Cloud — single-use 4-tile burst, CON save vs DC 15,
/// fail = `Poisoned` for 10 rounds (disadvantage on attacks / ability
/// checks). 5e RAW: level-3 conjuration, CON save, concentration; the
/// scroll collapses to a fixed-duration burst envelope and drops the
/// concentration gate. Fills the burst-Poisoned niche in the loot pool
/// alongside Pipes of Haunting (burst Frightened) and Wand of Web
/// (burst Restrained).
pub static SCROLL_OF_STINKING_CLOUD: Item = Item {
    name: "Scroll of Stinking Cloud",
    glyph: 'c',
    on_use: Some(&crate::actions::item_actions::READ_STINKING_CLOUD_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Death Ward — single-use Action; install `DeathWarded` on a
/// single ally for 100 rounds. 5e RAW: level-4 abjuration, action, touch,
/// 8-hour duration; the scroll collapses to the engine's standard fixed-
/// duration buff envelope. The ward absorbs the next lethal blow (any
/// damage that would drop the holder to 0 HP instead leaves them at 1)
/// and then burns off. Sits in the loot pool as a defensive ally-buff
/// scroll alongside Scroll of Bless / Scroll of Shield of Faith.
pub static SCROLL_OF_DEATH_WARD: Item = Item {
    name: "Scroll of Death Ward",
    glyph: 'W',
    on_use: Some(&crate::actions::item_actions::READ_DEATH_WARD_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Aid — single-use Action; bumps up to 3 allies' max HP and
/// current HP by 5 within a 4-tile burst. 5e RAW: level-2 abjuration,
/// 8-hour duration, max 3 targets within 30 ft; the scroll collapses to
/// a burst-targeted version that picks the lowest-HP allies first. Sits
/// in the loot pool as a multi-target permanent buff scroll alongside
/// Mass Healing Word (multi-ally instant heal).
pub static SCROLL_OF_AID: Item = Item {
    name: "Scroll of Aid",
    glyph: 'A',
    on_use: Some(&crate::actions::item_actions::READ_AID_SCROLL),
    ..Item::DEFAULTS
};

/// Wand of Binding — single-use Action; single-target DEX save vs DC 15,
/// fail = `Restrained` for 10 rounds. 5e flavor: a wand carved with iron
/// runes that fixes a target in place with crackling bands of force.
/// Single-target counterpart to the Wand of Web (burst Restrained).
/// Distinct loot tier from the Wand of Web — the burst hits more targets
/// but the wand is harder to dodge (single-target DC 15 vs burst DC 15).
pub static WAND_OF_BINDING: Item = Item {
    name: "Wand of Binding",
    glyph: 'B',
    on_use: Some(&crate::actions::item_actions::USE_WAND_OF_BINDING),
    ..Item::DEFAULTS
};

/// Scroll of Banishment — single-use Action; single-target CHA save vs
/// DC 15, fail = `Mazed` (banished demi-plane envelope) for 10 rounds.
/// 5e RAW: level-4 abjuration, concentration, 60-ft range; the scroll
/// drops concentration and uses a fixed 10-round timer. Sits at the top
/// of the single-target CC tier alongside Wand of Polymorph — both
/// effectively remove the target from the encounter.
pub static SCROLL_OF_BANISHMENT: Item = Item {
    name: "Scroll of Banishment",
    glyph: 'X',
    on_use: Some(&crate::actions::item_actions::READ_BANISHMENT_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Fear — single-use Action; 4-tile burst, WIS save vs DC 15,
/// fail = `Frightened` for 10 rounds. 5e RAW: level-3 illusion,
/// concentration, 30-ft cone; the scroll drops concentration and uses a
/// burst envelope. Harder DC version of Pipes of Haunting (burst
/// Frightened DC 13) — sits alongside Wand of Fear (single-target
/// Frightened DC 15) so the loot pool covers all three combinations of
/// (burst/single, soft/hard DC) on the Frightened lane.
pub static SCROLL_OF_FEAR: Item = Item {
    name: "Scroll of Fear",
    glyph: 'r',
    on_use: Some(&crate::actions::item_actions::READ_FEAR_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Charm Person — single-use Action; single-target, WIS save vs
/// DC 13, fail = `Charmed` for 10 rounds. 5e RAW: level-1 enchantment, 30-ft
/// range, 1-hour duration; the scroll drops concentration (Charm Person has
/// none anyway) and lands the engine's standard 10-round CC envelope. Entry-
/// level enchantment scroll alongside Scroll of Hold Person / Wand of Sleep.
/// Charm-immune families (undead, constructs, fiends) silently skip the save
/// through the up-front immunity filter in `SingleSaveConditionItem`.
pub static SCROLL_OF_CHARM_PERSON: Item = Item {
    name: "Scroll of Charm Person",
    glyph: 'p',
    on_use: Some(&crate::actions::item_actions::READ_CHARM_PERSON_SCROLL),
    ..Item::DEFAULTS
};

/// Wand of Charm Monster — single-use Action; single-target, WIS save vs DC
/// 15, fail = `Charmed` for 10 rounds. Top-tier enchantment consumable: same
/// shape as `SCROLL_OF_CHARM_PERSON` but a harder DC and longer reach (60 ft
/// vs 30 ft). 5e RAW: level-4 enchantment, no concentration, 1-hour duration.
/// Distinct from the scroll variant since Charm Monster lands on creature
/// types the Person variant can't reach — the engine doesn't gate by creature
/// type today, so the niche is the harder DC + longer reach.
pub static WAND_OF_CHARM_MONSTER: Item = Item {
    name: "Wand of Charm Monster",
    glyph: 'm',
    on_use: Some(&crate::actions::item_actions::USE_WAND_OF_CHARM_MONSTER),
    ..Item::DEFAULTS
};

/// Scroll of Tasha's Hideous Laughter — single-use Action; single-target,
/// WIS save vs DC 13, fail = `Incapacitated` for 10 rounds. 5e RAW: level-1
/// enchantment, 30-ft range, concentration, re-save each turn; the scroll
/// drops concentration and uses the engine's standard fixed-duration CC
/// envelope. Sits in the loot pool as the entry-level Incapacitated
/// installer alongside Scroll of Charm Person — both are WIS save vs DC 13.
pub static SCROLL_OF_TASHAS_HIDEOUS_LAUGHTER: Item = Item {
    name: "Scroll of Tasha's Hideous Laughter",
    glyph: 'L',
    on_use: Some(&crate::actions::item_actions::READ_TASHAS_HIDEOUS_LAUGHTER_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Heat Metal — single-use Bonus Action; single-target, CON save
/// vs DC 13, fail = `HeatMetaled` for 10 rounds (disadvantage on attack
/// rolls / ability checks). 5e RAW: level-2 transmutation, action, no save
/// on cast (CON save each turn to drop the gear); the scroll collapses to
/// a single up-front CON save vs the standard scroll DC. Drops the per-
/// round fire damage rider from the spell — the load-bearing combat clause
/// is the attack-roll disadvantage, which flows through the existing
/// `HeatMetaled` condition. Fires through the shared
/// `SingleSaveConditionItem` impl.
pub static SCROLL_OF_HEAT_METAL: Item = Item {
    name: "Scroll of Heat Metal",
    glyph: 'h',
    on_use: Some(&crate::actions::item_actions::READ_HEAT_METAL_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Ice Storm — single-use Action; 4-tile burst, DEX save vs DC 15,
/// fail = 4d8 cold, pass = half. 5e RAW: level-4 evocation that deals
/// 2d8 bludgeoning + 4d6 cold; we collapse the dual-type damage to a single
/// cold roll (4d8) so the scroll fires through the shared
/// `BurstSaveDamageItem` impl. Sits between Wand of Cone of Cold (10d8 cold,
/// 6-radius) and the scroll-tier cold burst as the mid-tier cold-burst
/// scroll.
pub static SCROLL_OF_ICE_STORM: Item = Item {
    name: "Scroll of Ice Storm",
    glyph: 'I',
    on_use: Some(&crate::actions::item_actions::READ_ICE_STORM_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Web — single-use Action; 4-tile burst, DEX save vs DC 13, fail
/// = `Restrained` for 10 rounds. 5e RAW: level-2 conjuration, concentration,
/// 60-ft range / 20-ft cube, no concentration on the scroll. Sibling to
/// Wand of Web (DC 15 Restrained burst) at the cheap-tier weight — same
/// shape, easier DC. Both ride the shared `BurstSaveConditionItem` impl.
pub static SCROLL_OF_WEB: Item = Item {
    name: "Scroll of Web",
    glyph: 'W',
    on_use: Some(&crate::actions::item_actions::READ_WEB_SCROLL),
    ..Item::DEFAULTS
};

/// Potion of Resistance — Action; installs `DamageResistant` (halve all
/// incoming damage) for 10 rounds. 5e RAW grants resistance to a single
/// damage type; we collapse to the engine's blanket `DamageResistant`
/// envelope every other resistance potion (Fire / Cold / Stoneskin) rides.
/// Single-use; rejects re-drink when already resistant. Sits in the loot
/// pool as the un-flavored generic counterpart to the typed resistance
/// potions — the player can grab whichever flavor they roll without
/// stratifying the buff itself.
pub static POTION_OF_RESISTANCE: Item = Item {
    name: "Potion of Resistance",
    glyph: 'R',
    on_use: Some(&crate::actions::item_actions::DRINK_POTION_OF_RESISTANCE),
    ..Item::DEFAULTS
};

/// Potion of Vigilance — Bonus Action; installs `DangerSense` (advantage on
/// DEX saves while not Blinded / Incapacitated / Deafened) for 10 rounds.
/// Consumable counterpart to the passive `AMULET_OF_THE_VIGILANT`: the
/// amulet is always-on, the potion is a single-shot 10-round buff. Fits the
/// "AoE survival burst" niche for low-DEX casters who can't afford an
/// amulet slot. Single-use; rejects re-drink when already active.
pub static POTION_OF_VIGILANCE: Item = Item {
    name: "Potion of Vigilance",
    glyph: 'v',
    on_use: Some(&crate::actions::item_actions::DRINK_POTION_OF_VIGILANCE),
    ..Item::DEFAULTS
};

/// Necklace of Fireballs — Action; 5d6 fire DEX-save burst (DC 15)
/// centered on a target tile within 60 ft. Single-bead consumable (RAW
/// has a multi-bead necklace ladder; we collapse to a single-use scroll-
/// style envelope so the loot pool stays simple). Fires through the
/// shared `BurstSaveDamageItem` impl. Sits between Scroll of Fireball
/// (6d6) and Wand of Fireballs (8d6) — same envelope, smaller payload
/// to mark the bead-tier of the magic-fireball ladder.
pub static NECKLACE_OF_FIREBALLS: Item = Item {
    name: "Necklace of Fireballs",
    glyph: 'N',
    on_use: Some(&crate::actions::item_actions::USE_NECKLACE_OF_FIREBALLS),
    ..Item::DEFAULTS
};

/// Dust of Disappearance — Bonus Action; installs the `Invisible`
/// condition on the holder for 10 rounds. 5e RAW: 2d4 minutes invisible;
/// the engine collapses to the standard combat-scale 10-round timer
/// every Invisibility-flavored consumable rides. Counterpart to Potion
/// of Invisibility (Action cost) — the dust trades action lane for the
/// bonus-action quickness. Fires through the shared `SelfConditionItem`
/// impl.
pub static DUST_OF_DISAPPEARANCE: Item = Item {
    name: "Dust of Disappearance",
    glyph: 'd',
    on_use: Some(&crate::actions::item_actions::USE_DUST_OF_DISAPPEARANCE),
    ..Item::DEFAULTS
};

/// Wand of Suggestion — Action; single-target WIS save vs DC 15, fail =
/// `Charmed` for 10 rounds. 5e RAW: level-2 enchantment, concentration,
/// 30-ft range; the wand collapses to the standard fixed-duration
/// consumable envelope and drops the concentration gate. Sibling to
/// Wand of Charm Monster (also Charmed, also DC 15) — different reach
/// (24 vs 30 tiles RAW) so the loot pool has two flavored entries on
/// the same condition lane. Fires through the shared
/// `SingleSaveConditionItem` impl.
pub static WAND_OF_SUGGESTION: Item = Item {
    name: "Wand of Suggestion",
    glyph: 'u',
    on_use: Some(&crate::actions::item_actions::USE_WAND_OF_SUGGESTION),
    ..Item::DEFAULTS
};

/// Scroll of Calm Emotions — Action; 4-tile burst, CHA save vs DC 13,
/// fail = `Charmed` for 10 rounds. 5e RAW: level-2 enchantment,
/// concentration, two-option toggle (suppress fear OR Charm). The
/// engine collapses to the Charm-installer half (the engine-relevant
/// combat clause) and drops the concentration gate. Burst counterpart
/// to Scroll of Charm Person (single-target, same DC) and
/// debuff-burst-AoE counterpart to Scroll of Bane (CHA-save burst).
/// Fires through the shared `BurstSaveConditionItem` impl.
pub static SCROLL_OF_CALM_EMOTIONS: Item = Item {
    name: "Scroll of Calm Emotions",
    glyph: 's',
    on_use: Some(&crate::actions::item_actions::READ_CALM_EMOTIONS_SCROLL),
    ..Item::DEFAULTS
};

/// Wand of Blindness — Action; single-target CON save vs DC 15, fail =
/// `Blinded` for 10 rounds. 5e RAW: level-2 necromancy (Blindness /
/// Deafness), 30-ft range, CON save; the wand bumps the DC to the
/// standard wand tier (15) and the reach to 24 tiles (60 ft RAW). Top-
/// tier counterpart to Scroll of Blindness (CON save, DC 13) on the
/// single-target Blinded lane. Fires through the shared
/// `SingleSaveConditionItem` impl.
pub static WAND_OF_BLINDNESS: Item = Item {
    name: "Wand of Blindness",
    glyph: 'b',
    on_use: Some(&crate::actions::item_actions::USE_WAND_OF_BLINDNESS),
    ..Item::DEFAULTS
};

/// Scroll of Mass Cure Wounds — Action; touches every ally within a
/// 4-tile burst centered on the caster (RAW: 3d8 + spellcasting modifier
/// per ally; the scroll uses a flat 3d8+5 per ally). 5e RAW: level-5
/// conjuration, 60-ft range, up to 6 targets in a 30-ft radius. The
/// scroll collapses to a self-centered burst with the standard
/// 4-tile envelope and reuses the existing mass-heal item lane via the
/// hand-rolled `READ_MASS_HEALING_WORD_SCROLL` (no shared factor yet for
/// this exact shape — Mass Cure Wounds heals more per ally but uses an
/// Action cost vs the bonus-action Healing Word variant).
pub static SCROLL_OF_MASS_CURE_WOUNDS: Item = Item {
    name: "Scroll of Mass Cure Wounds",
    glyph: 'X',
    on_use: Some(&crate::actions::item_actions::READ_MASS_CURE_WOUNDS_SCROLL),
    ..Item::DEFAULTS
};

/// Boots of Levitation — passive trinket. Wearer is treated as Flying
/// (the engine's binary flight model). 5e RAW: action toggle to
/// levitate for up to 10 minutes; the engine collapses to a permanent
/// always-on flight install via the `passive_conditions` lane so the
/// boots sit alongside Winged Boots (also Flying) and Slippers of
/// Spider Climbing on the mobility-trinket lane. Difference from Winged
/// Boots: cosmetic (vertical-only vs full flight RAW) — both grant the
/// same in-engine `Flying` flag, so the loot pool keeps two flavored
/// entries on the same mechanical envelope.
pub static BOOTS_OF_LEVITATION: Item = Item {
    name: "Boots of Levitation",
    glyph: 'L',
    passive_conditions: &[crate::conditions::Condition::Flying],
    ..Item::DEFAULTS
};

/// Cloak of Elvenkind — passive trinket. Grants the wearer the
/// `Untracked` condition (Pass Without Trace's +10 stealth-flavored
/// rider, modeled in the engine as a flat to-hit-vs-the-wearer
/// disadvantage chokepoint). 5e RAW: "creatures that try to spot you
/// have disadvantage on Wisdom (Perception) checks" — the engine
/// collapses Perception to the attack-against-the-wearer disadvantage
/// since stealth-as-cover-for-the-next-swing is the load-bearing
/// in-combat consequence. Sits alongside Cloak of Displacement on the
/// "attacker-disadvantage" trinket lane.
pub static CLOAK_OF_ELVENKIND: Item = Item {
    name: "Cloak of Elvenkind",
    glyph: 'e',
    passive_conditions: &[crate::conditions::Condition::Untracked],
    ..Item::DEFAULTS
};

/// Ring of Spell Storing — Action; the ring discharges into a Magic-
/// Missile-style auto-hit dart against a single target (3 darts ×
/// 1d4+1 force, no save). Single-use; we collapse the RAW "stored
/// spells" subsystem (which would require an Action / SpellSlot ledger
/// on the trinket) to a fixed force-dart payload, matching the size of
/// the level-1 Magic Missile scroll. Fires through the shared
/// `MagicMissileItem` impl.
pub static RING_OF_SPELL_STORING: Item = Item {
    name: "Ring of Spell Storing",
    glyph: 'S',
    on_use: Some(&crate::actions::item_actions::USE_RING_OF_SPELL_STORING),
    ..Item::DEFAULTS
};

/// Scroll of Cloudkill — Action; 4-tile burst, CON save vs DC 15, fail =
/// 5d8 poison damage. 5e RAW: level-5 conjuration, 40-ft moving cloud
/// dealing 5d8 poison; the scroll collapses the moving-cloud lane to a
/// single one-shot burst-damage roll. Sits in the rare half of the
/// poison-burst lane (the only poison-damage burst consumable; Stinking
/// Cloud uses poison-CONDITION rather than poison-DAMAGE). Fires through
/// the shared `BurstSaveDamageItem` impl.
pub static SCROLL_OF_CLOUDKILL: Item = Item {
    name: "Scroll of Cloudkill",
    glyph: 'C',
    on_use: Some(&crate::actions::item_actions::READ_CLOUDKILL_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Prayer of Healing — Action; self-centered burst that heals
/// up to 6 nearest allies for 2d8+3 HP each (within 6 tiles). 5e RAW:
/// level-2 evocation; 10-min cast time and 30-ft range. The scroll
/// collapses the 10-min cast to an Action and uses a tighter ally pick
/// envelope. Sits between Mass Healing Word (1d4+3 bonus action) and
/// Mass Cure Wounds (3d8+5 action) on the multi-target heal ladder.
/// Fires through the shared `MultiTargetHealItem` impl.
pub static SCROLL_OF_PRAYER_OF_HEALING: Item = Item {
    name: "Scroll of Prayer of Healing",
    glyph: 'P',
    on_use: Some(&crate::actions::item_actions::READ_PRAYER_OF_HEALING_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Greater Cure Wounds — Action; 4d8+5 single-target heal at
/// touch range. Mid-tier between Scroll of Cure Wounds (2d8+2) and the
/// Wand of Greater Healing (4d8+4) on the single-target ally heal lane.
/// 5e RAW: Cure Wounds upcast at level 4 = 5d8 + caster mod; we collapse
/// to 4d8+5 to slot cleanly between the scroll and wand tiers. Fires
/// through the shared `SingleTargetHealItem` impl.
pub static SCROLL_OF_GREATER_CURE_WOUNDS: Item = Item {
    name: "Scroll of Greater Cure Wounds",
    glyph: 'g',
    on_use: Some(&crate::actions::item_actions::READ_GREATER_CURE_WOUNDS_SCROLL),
    ..Item::DEFAULTS
};

/// Potion of Haste — Bonus Action; installs `Hasted` on the holder for
/// 10 rounds (+2 AC, advantage on DEX saves, doubled walking speed).
/// Distinct from Potion of Speed (extra Action this turn plus a flat
/// +1 attack/save): Haste rides the engine's `Hasted` condition for the
/// AC / DEX-save / speed bundle. Fires through the shared
/// `SelfConditionItem` impl. Rejects re-drink when already Hasted.
pub static POTION_OF_HASTE: Item = Item {
    name: "Potion of Haste",
    glyph: 'H',
    on_use: Some(&crate::actions::item_actions::DRINK_POTION_OF_HASTE),
    ..Item::DEFAULTS
};

/// Scroll of Flesh to Stone — Action; single-target CON save vs DC 15,
/// fail = `Petrified` for 10 rounds. 5e RAW: level-6 transmutation,
/// concentration, three-save ladder; the scroll collapses to a single
/// save-or-stone install and drops concentration. Top of the single-
/// target lockdown ladder — Petrified blocks the action economy AND
/// drops the target's AC against physical damage (auto-fail STR / DEX
/// saves). Fires through the shared `SingleSaveConditionItem` impl.
pub static SCROLL_OF_FLESH_TO_STONE: Item = Item {
    name: "Scroll of Flesh to Stone",
    glyph: 'F',
    on_use: Some(&crate::actions::item_actions::READ_FLESH_TO_STONE_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Synaptic Static — 8d6 psychic-damage burst at DC 15 INT save.
/// Fills the psychic burst-damage niche in the scroll family alongside
/// Fire / Lightning / Cold / Acid / Thunder / Poison. Fires through the
/// shared `BurstSaveDamageItem` action impl.
pub static SCROLL_OF_SYNAPTIC_STATIC: Item = Item {
    name: "Scroll of Synaptic Static",
    glyph: 'y',
    on_use: Some(&crate::actions::item_actions::READ_SYNAPTIC_STATIC_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Circle of Death — 8d6 necrotic-damage burst at DC 15 CON save.
/// The only necrotic-damage burst consumable in the loot pool; sits in
/// the rare half of the lane alongside Synaptic Static (psychic) as a
/// premium typed-burst scroll. Fires through the shared
/// `BurstSaveDamageItem` action impl.
pub static SCROLL_OF_CIRCLE_OF_DEATH: Item = Item {
    name: "Scroll of Circle of Death",
    glyph: 'O',
    on_use: Some(&crate::actions::item_actions::READ_CIRCLE_OF_DEATH_SCROLL),
    ..Item::DEFAULTS
};

/// Potion of Mind Blank — installs `MindBlanked` on the drinker for 10
/// rounds. Top-tier mental-defense consumable: immune to psychic damage
/// AND immune to the Charmed condition for the duration. Sits in the
/// rare half of the loot pool alongside Periapt of Proof against Poison
/// (poison immunity passive) on the typed-immunity lane. Fires through
/// the shared `SelfConditionItem` action impl.
pub static POTION_OF_MIND_BLANK: Item = Item {
    name: "Potion of Mind Blank",
    glyph: 'M',
    on_use: Some(&crate::actions::item_actions::DRINK_POTION_OF_MIND_BLANK),
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
    // Potion of Longstrider — cheap mobility consumable. Same low-weight
    // tier as Potion of Climbing; +10 ft for the encounter. Bonus-action
    // drink so it doesn't compete with the holder's main turn budget.
    &POTION_OF_LONGSTRIDER,
    // Ally-buff scrolls — Bless and Shield of Faith fill the support
    // scroll niche alongside the Mass Healing Word / Cure Wounds heal
    // scrolls. Both buff a single ally for 10 rounds; Bless boosts
    // attack rolls / saves (+1d4), Shield of Faith boosts AC (+2).
    &SCROLL_OF_BLESS,
    &SCROLL_OF_SHIELD_OF_FAITH,
    // Single-target CC scrolls — Blindness (CON save Blinded) fills the
    // entry-level "save-or-suck single target" niche alongside the rare
    // Wand of Paralysis. Wand of Polymorph sits at the top of the same
    // ladder — Polymorphed locks out the target's spellcasting toolkit.
    &SCROLL_OF_BLINDNESS,
    &WAND_OF_POLYMORPH,
    // Burst CC scrolls — Bane (CHA save Baned) is the burst debuff
    // counterpart to Bless; Faerie Fire (DEX save Outlined) sets up the
    // martial-heavy party for advantage on follow-up swings.
    &SCROLL_OF_BANE,
    &SCROLL_OF_FAERIE_FIRE,
    // Defensive consumables — Barkskin (AC 16 floor), Fire / Cold
    // Resistance potions (blanket damage halve), and Hill Giant Strength
    // (Enlarged size+damage). Single entries each; the resistance
    // potions share the `DamageResistant` lane with Potion of Stoneskin
    // so the loot pool has multiple flavored entries on the same
    // mechanical envelope.
    &POTION_OF_BARKSKIN,
    &POTION_OF_FIRE_RESISTANCE,
    &POTION_OF_COLD_RESISTANCE,
    &POTION_OF_HILL_GIANT_STRENGTH,
    // Passive-condition trinkets — Slippers of Spider Climbing
    // (SpiderClimbing speed bump), Winged Boots (Flying), Boots of the
    // Forest (Longstriding +10 ft), Cloak of Etherealness (DamageResistant
    // blanket halve), Amulet of the Vigilant (DangerSense DEX-save
    // advantage). All ride the new `passive_conditions` item lane so the
    // install lives on `pickup_item` and persists across long rests via
    // the `reinstall_item_passive_conditions` hook.
    &SLIPPERS_OF_SPIDER_CLIMBING,
    &WINGED_BOOTS,
    &BOOTS_OF_THE_FOREST,
    &CLOAK_OF_ETHEREALNESS,
    &AMULET_OF_THE_VIGILANT,
    // Cloak of Displacement — rare passive trinket with the Displaced
    // condition rider. Pairs with the Amulet of the Vigilant in the
    // "save-flavored" trinket niche on a different defensive lane
    // (attacker-disadvantage instead of save-advantage).
    &CLOAK_OF_DISPLACEMENT,
    // Scarab of Protection — legendary-tier dual-immunity trinket plus a
    // small flat save bump. Sits in the rare half of the pool alongside
    // the Periapt of Proof against Poison / Ring of Mind Shielding
    // dual-lane trinkets.
    &SCARAB_OF_PROTECTION,
    // Ring of Heroism — passive-condition counterpart to Potion of Heroism;
    // single low-weight entry alongside Slippers / Winged Boots.
    &RING_OF_HEROISM,
    // Wand of Sleep — entry-tier single-target CC (DC 13 Asleep). Lighter
    // tier than Scroll of Hold Person (DC 13 Paralyzed) since Asleep
    // breaks on damage; the loot pool keeps both as single-entry
    // alternatives for the rogue / fighter's "first knockout swing"
    // niche.
    &WAND_OF_SLEEP,
    // Burst-debuff scrolls — Slow (WIS save, AC/movement penalty) and
    // Stinking Cloud (CON save, Poisoned blanket). Single entries each
    // alongside Scroll of Bane / Faerie Fire on the burst-debuff lane.
    &SCROLL_OF_SLOW,
    &SCROLL_OF_STINKING_CLOUD,
    // Defensive ally-buff scrolls — Death Ward (next-lethal-blow absorb)
    // and Aid (multi-ally permanent +5 max HP / current HP). Single
    // entries each in the rare half of the support-scroll lane.
    &SCROLL_OF_DEATH_WARD,
    &SCROLL_OF_AID,
    // Top-tier single-target CC consumables — Banishment (CHA save
    // banished-to-demiplane) and Binding (DEX save Restrained). Single
    // entries each in the rare half of the CC-consumable lane.
    &SCROLL_OF_BANISHMENT,
    &WAND_OF_BINDING,
    // Scroll of Fear — burst Frightened at the rare DC 15 tier. Sits
    // between Pipes of Haunting (burst DC 13) and Wand of Fear (single-
    // target DC 15).
    &SCROLL_OF_FEAR,
    // Charm consumables — single-target Charmed at the cheap (DC 13) and
    // rare (DC 15) tiers. Sit in the loot pool as the enchantment lane
    // counterparts to the Hold Person / Hold Monster Paralyzed family.
    &SCROLL_OF_CHARM_PERSON,
    &WAND_OF_CHARM_MONSTER,
    // Tasha's Hideous Laughter — entry-tier single-target Incapacitated
    // installer. Same WIS-save-DC-13 envelope as Charm Person but a
    // different lockdown condition (Incapacitated blocks actions; Charmed
    // is marker-only today).
    &SCROLL_OF_TASHAS_HIDEOUS_LAUGHTER,
    // Heat Metal scroll — single-target HeatMetaled (attack-roll
    // disadvantage) at the cheap CON-save tier. Distinct from the burst
    // debuff scrolls (Bane / Slow / Stinking Cloud) — single-target,
    // attack-roll-targeted.
    &SCROLL_OF_HEAT_METAL,
    // Ice Storm scroll — mid-tier cold burst between Cone of Cold (8d8)
    // and Shatter (3d8 thunder). Single entry — same rarity as the
    // existing cold-burst Cone of Cold scroll.
    &SCROLL_OF_ICE_STORM,
    // Web scroll — cheap-tier counterpart to Wand of Web (DC 15
    // Restrained burst). Same shape, easier DC; sits alongside Pipes of
    // Haunting (burst Frightened DC 13) at the entry-level burst CC tier.
    &SCROLL_OF_WEB,
    // Generic / un-flavored Potion of Resistance — DamageResistant for
    // 10 rounds. Drops alongside the flavored Fire / Cold variants for
    // a third roll on the same envelope.
    &POTION_OF_RESISTANCE,
    // Vigilance potion — consumable counterpart to Amulet of the Vigilant.
    // Single low-weight entry; installs the DangerSense condition.
    &POTION_OF_VIGILANCE,
    // Necklace of Fireballs — sub-tier fire burst (5d6, between scroll's
    // 6d6 and wand's 8d6). Single entry alongside the rest of the fire-
    // burst consumable family.
    &NECKLACE_OF_FIREBALLS,
    // Dust of Disappearance — bonus-action Invisibility counterpart to
    // Potion of Invisibility (Action cost). Single entry.
    &DUST_OF_DISAPPEARANCE,
    // Wand of Suggestion — single-target Charmed at the rare DC 15 tier.
    // Sibling to Wand of Charm Monster on the Charmed lane.
    &WAND_OF_SUGGESTION,
    // Scroll of Calm Emotions — burst Charmed at the cheap DC 13 tier;
    // burst counterpart to Scroll of Charm Person (single, same DC).
    &SCROLL_OF_CALM_EMOTIONS,
    // Wand of Blindness — single-target Blinded at the rare DC 15 tier.
    // Top-tier counterpart to Scroll of Blindness (CON save DC 13).
    &WAND_OF_BLINDNESS,
    // Scroll of Mass Cure Wounds — burst ally heal at Action cost.
    // Pairs with Mass Healing Word scroll (bonus-action variant) on the
    // mass-heal scroll lane.
    &SCROLL_OF_MASS_CURE_WOUNDS,
    // Boots of Levitation — passive Flying trinket. Sibling to Winged
    // Boots on the always-on flight lane.
    &BOOTS_OF_LEVITATION,
    // Cloak of Elvenkind — passive Untracked trinket. Attacker-disadvantage
    // counterpart to Cloak of Displacement; lower weight as a single entry.
    &CLOAK_OF_ELVENKIND,
    // Ring of Spell Storing — single-use Magic-Missile-style force dart
    // volley. Sibling to Scroll of Magic Missile on the auto-hit lane.
    &RING_OF_SPELL_STORING,
    // Scroll of Cloudkill — 5d8 poison-damage burst at DC 15. The only
    // poison-damage burst consumable; sits alongside Stinking Cloud
    // (poison-CONDITION) on the poison-flavored AoE lane.
    &SCROLL_OF_CLOUDKILL,
    // Scroll of Prayer of Healing — mid-tier multi-ally heal between
    // Mass Healing Word (1d4+3 bonus action) and Mass Cure Wounds
    // (3d8+5 action). Single entry on the multi-target heal lane.
    &SCROLL_OF_PRAYER_OF_HEALING,
    // Scroll of Greater Cure Wounds — 4d8+5 single-target touch heal.
    // Slots between the cheap Cure Wounds scroll and the Wand of
    // Greater Healing on the ally-heal ladder.
    &SCROLL_OF_GREATER_CURE_WOUNDS,
    // Potion of Haste — Hasted install on a bonus action. Distinct from
    // Potion of Speed (extra-action burst); same condition envelope as
    // the Boots of Speed but bonus-action timing.
    &POTION_OF_HASTE,
    // Scroll of Flesh to Stone — top-tier single-target Petrified
    // installer. Single entry alongside Wand of Polymorph on the
    // rare half of the single-target lockdown lane.
    &SCROLL_OF_FLESH_TO_STONE,
    // Scroll of Synaptic Static — 8d6 psychic-damage burst at DC 15 INT
    // save. Fills the psychic-burst niche between Cone of Cold (cold)
    // and Vitriolic Sphere (acid) — single entry alongside the rare
    // typed-burst scroll tier.
    &SCROLL_OF_SYNAPTIC_STATIC,
    // Scroll of Circle of Death — 8d6 necrotic-damage burst at DC 15 CON
    // save. The only necrotic-typed burst in the loot pool; rounds out
    // the typed-burst scroll family.
    &SCROLL_OF_CIRCLE_OF_DEATH,
    // Potion of Mind Blank — top-tier mental-defense consumable.
    // Psychic + Charmed immunity for 10 rounds. Single low-weight entry
    // alongside the other premium typed-defense consumables.
    &POTION_OF_MIND_BLANK,
];
