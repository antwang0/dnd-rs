/// Persistent status effects an actor can have. Mechanical impact lives
/// at the consumer (e.g. `ActorInstance::remaining_movement` returns 0
/// while `Prone`; `can_consume_resource` blocks Action/BonusAction/Reaction
/// while `Stunned`). New variants land here and then plug into the
/// relevant accessor — no central dispatcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Condition {
    /// Speed = 0; ranged attacks against you have disadvantage; melee
    /// against you have advantage; you have disadvantage on attacks.
    Prone,
    /// Cannot take Actions, Bonus Actions, or Reactions. Movement is also
    /// 0 (in 5e via Incapacitated, but we collapse for simplicity). Attacks
    /// vs you have advantage; auto-fail STR/DEX saves.
    Stunned,
    /// Disadvantage on attack rolls and ability checks.
    Poisoned,
    /// Disadvantage on attack rolls and ability checks while you can see
    /// the source of fear. We don't track LOS-to-fear-source today; the
    /// effect is unconditional disadvantage on attacks.
    Frightened,
    /// Speed = 0; disadvantage on attacks; attacks against you have
    /// advantage; disadvantage on DEX saves.
    Restrained,
    /// Auto-fail any check requiring sight; attack rolls against you have
    /// advantage; you have disadvantage on attacks.
    Blinded,
    /// Cannot take actions or reactions (movement is still allowed).
    /// Distinct from Stunned — Incapacitated still moves; Stunned can't.
    Incapacitated,
    /// You have no effect on combat behavior — you can't take hostile
    /// actions against the charmer. Marker only today; we don't yet
    /// model the "can't attack the charmer" enforcement.
    Charmed,
    /// Cannot move; cannot take actions or reactions; auto-fail STR/DEX
    /// saves; attacks against you have advantage; melee crits are
    /// auto-criticals on hit. We model the action-economy / movement
    /// blocks here; the auto-crit on melee hit lives at the attack site.
    Paralyzed,
    /// Cannot be seen without special vision. Attack rolls against you
    /// have disadvantage; your attacks have advantage. Bookkept as a
    /// condition for clean removal on attack (per 5e Greater Invisibility
    /// vs Invisibility — we treat both as the simple Invisible condition).
    Invisible,
    /// Active until the start of your next turn after taking the Dodge
    /// action: attacks against you have disadvantage (if you can see the
    /// attacker); you have advantage on DEX saves. Cleared by the
    /// `UntilStartOfNextTurn` timer on your next turn.
    Dodging,
    /// Speed = 0; you can't gain a speed bonus. Ends when grappler is
    /// incapacitated or target is moved out of range. We track only the
    /// movement block here; ending is up to the grappler logic.
    Grappled,
    /// +1d4 to attack rolls and saving throws (Bless spell). Tracked as a
    /// condition so it ticks down with the spell timer and clears cleanly
    /// when concentration drops.
    Blessed,
    /// +2 AC from Shield of Faith (concentration buff). Read by
    /// `armor_class()`; drops when concentration drops.
    ShieldOfFaith,
    /// +5 AC from the Shield reaction spell, until the start of your
    /// next turn. Cleared by the `UntilStartOfNextTurn` timer.
    Shielded,
    /// Generic "halve incoming damage" buff (e.g. Stoneskin).
    /// Stacks multiplicatively with damage-type resistance.
    DamageResistant,
    /// Hostile-to-hostile attack against the holder is at advantage and
    /// the holder cannot benefit from being Hidden/Invisible. Used by
    /// Faerie Fire / Hunter's Mark style effects.
    Outlined,
    /// Lit by Guiding Bolt — next attack against this actor before the
    /// end of the caster's next turn has advantage. Burns off when the
    /// next attack lands or when its short timer expires.
    GuidingBoltLit,
    /// Took the Help action against this turn — your next attack against
    /// the helped target has advantage (cleared on use or end of round).
    Helped,
    /// Successful Stealth check; you have unseen advantage on attack and
    /// attackers have disadvantage. Distinct from Invisible: it's broken
    /// by attacking, ending hidden status.
    Hidden,
    /// On fire — takes 1d4 fire at the start of each of its turns until
    /// extinguished. Burning is a DOT condition with `Rounds(n)` timer.
    Burning,
    /// Took the Disengage action this turn: their movement doesn't
    /// provoke opportunity attacks. Cleared by `UntilStartOfNextTurn`.
    Disengaging,
    /// Knocked unconscious (HP 0 or magical sleep). Stronger than
    /// Incapacitated: drops prone, fails STR/DEX saves, and melee crits
    /// on hit. Set automatically when an actor enters HpState::Dying.
    Unconscious,
    /// -1d4 (modeled as -2) to attack rolls and saving throws (Bane spell).
    /// Symmetric counterpart to `Blessed`. Tracked as a condition so it
    /// ticks down with the spell timer and clears on concentration drop.
    Baned,
    /// Cannot hear; auto-fail any check requiring hearing. We don't yet
    /// model verbal-component spell failure or audio-based perception, so
    /// the in-combat impact is mostly cosmetic — but the flag is here for
    /// spells like Blindness/Deafness so they can apply something.
    Deafened,
    /// Marked by Hunter's Mark — the marker (concentrating caster) deals
    /// an extra 1d6 weapon damage to this target. Tracked as a condition
    /// so dropping concentration cleans it up automatically.
    HuntersMarked,
    /// Mage Armor active — base AC becomes 13 + DEX modifier (we model as
    /// a flat AC boost via `condition_ac_bonus`). Lasts 8 hours; we just
    /// give it a long Rounds timer.
    MageArmored,
    /// Mocked by Vicious Mockery — disadvantage on the next attack roll
    /// before the end of the target's next turn. Short timer
    /// (`UntilStartOfNextTurn`) clears the debuff after the holder takes
    /// their turn (the disadvantage applies to attacks they make while
    /// the condition is up).
    Mocked,
    /// Heroism active — immune to Frightened and gaining temp HP each
    /// round from the spell's caster. Tracked as a condition so it
    /// clears cleanly on concentration drop.
    Heroic,
    /// Cannot cast spells / take reactions until the start of their next
    /// turn (Shocking Grasp's rider on a hit against a creature wearing
    /// metal armor — we model the simpler "no reaction" clause). Clears
    /// at start of own next turn.
    NoReaction,
    /// Asleep (5e Sleep spell). Same mechanical effect as Unconscious
    /// (prone, can't act, attacks against have advantage with auto-crit on
    /// melee hit), but distinguished from natural unconsciousness so it
    /// can be removed by taking damage or being shaken awake (any non-zero
    /// damage we receive while Asleep wakes us; modeled by stripping the
    /// condition on the damage-application path).
    Asleep,
    /// Surrounded by shimmering duplicates (5e Mirror Image). The holder
    /// has a small pool of decoys (we track count via `mirror_images()`
    /// on the actor); each incoming attack against them has a chance to
    /// hit a duplicate instead, popping one. Drops to 0 ends the spell.
    /// Doesn't require concentration.
    MirroredImages,
    /// Protected by Protection from Evil and Good. Aberrations, celestials,
    /// elementals, fey, fiends, and undead have disadvantage on attacks
    /// against this target. We approximate by giving disadvantage to *any*
    /// attacker that is undead-flavored (i.e. has the Poisoned condition
    /// immunity that mortal humanoids lack) — close enough for our pool of
    /// fiends / undead / etc.
    Warded,
    /// Hexed by the warlock Hex spell. The hex's caster deals +1d6 necrotic
    /// on weapon attacks against this target. Tracked as a condition so
    /// dropping concentration cleans it up automatically — symmetric with
    /// Hunter's Mark.
    Hexed,
    /// Blurred (5e Blur spell, concentration). Attacks against the holder
    /// have disadvantage unless the attacker can ignore the effect (no
    /// modeling of blindsight overrides — kept simple). Cleared when the
    /// caster drops concentration.
    Blurred,
    /// Stuck fast (5e Mimic Adhesive on hit). Functionally similar to
    /// Grappled but the catcher is the mimic itself; we reuse the
    /// movement-zero mechanic and keep this distinct so the log makes
    /// the cause obvious.
    Adhered,
    /// Hasted (5e Haste spell, concentration). +2 AC, advantage on DEX
    /// saves, doubled walking speed. We don't model the extra-action
    /// rider (action economy stays one Action per turn) — the AC + DEX
    /// save half is the load-bearing part for survivability and the
    /// doubled speed lets the holder reposition aggressively. Cleared
    /// when the caster's concentration ends.
    Hasted,
    /// Slowed (5e Slow spell). Halved walking speed, -2 AC, -2 DEX
    /// saves. The 5e spell also halves the holder's action economy
    /// (no reactions, can only cast a 1-action spell *or* attack);
    /// we model the static half — AC + DEX hit + movement — and skip
    /// the action-economy clause to avoid surprising the AI. Tracked
    /// as a condition so it clears cleanly on concentration drop.
    Slowed,
    /// Warded by Death Ward — the next time the holder would drop to
    /// 0 HP, they instead drop to 1 HP and the condition burns off.
    /// Damage that *would* kill outright (massive damage > max HP at 0)
    /// is also absorbed by the ward. Implemented in
    /// `ActorInstance::take_damage` so any damage path benefits — the
    /// ward intercepts before death-save / Dead transitions, then
    /// removes itself so a second killing blow lands as normal.
    DeathWarded,
    /// Petrified — turned to inanimate stone (5e Cockatrice / Flesh to
    /// Stone). Movement zero, action economy blocked (same envelope as
    /// Stunned / Paralyzed). 5e also adds wide damage resistance and
    /// poison immunity, plus auto-fail STR/DEX saves; we model the
    /// auto-fail via the existing `auto_fail_save` switch (extended to
    /// recognize Petrified alongside Paralyzed / Stunned).
    Petrified,
    /// Sanctuary (5e Sanctuary spell, level-1 abjuration, bonus action).
    /// Any creature targeting the warded actor with an attack or harmful
    /// spell must succeed on a WIS save vs the caster's spell DC or the
    /// effect fails (and the attacker can't target the warded for the
    /// rest of the turn). We model this by gating hostile actions inside
    /// `Action::validate_input`: the attacker rolls a one-shot WIS save
    /// at attack-roll time; on fail, the action silently no-ops. The
    /// warded actor loses the buff the moment they themselves attack or
    /// cast a damaging spell — the engine clears the condition on the
    /// holder's first hostile action.
    Sanctuary,
    /// Fire Shield — warm version (5e Fire Shield spell, level-4
    /// evocation). The holder gains resistance to cold damage (we
    /// approximate with the generic `DamageResistant` model — see
    /// `Stoneskin`). Any creature within reach that hits the holder
    /// takes 2d8 fire damage in retaliation (handled in
    /// `EncounterInstance::resolve_attack` via the fire-shield reflect
    /// hook). Concentration-free — fire shields are short-duration
    /// auto-clear via the Rounds timer.
    FireShielded,
    /// Daylit (5e Daylight spell, level-3 evocation). The actor sits
    /// inside a 60-foot sphere of bright light. We model this as a
    /// passive buff that gives undead / fiends (proxied by Necrotic /
    /// Poison immunity) disadvantage on attacks targeting the holder —
    /// mirroring `Warded` (Protection from Evil and Good) but for an
    /// area-buff rather than a single-target buff. Lasts for the spell's
    /// duration (1 hour RAW; we cap to a long Rounds timer).
    Daylit,
    /// Spike Growth (5e level-2 transmutation, concentration). The target
    /// is standing in spiked terrain — any movement they make this turn
    /// will deal 2d4 piercing damage per 5ft moved. We approximate by
    /// tagging the holder with the condition; the damage rider lives on
    /// the `MoveActor` apply path which reads this flag and bills the
    /// caster's spike damage once per step.
    Spiked,
    /// Raging (5e Barbarian feature). +2 melee damage on STR-based attacks,
    /// resistance to bludgeoning / piercing / slashing damage (we use the
    /// generic `DamageResistant` model in parallel), and advantage on STR
    /// checks / saves. Active for 10 rounds (an approximation of the
    /// 5e 1-minute duration). Bonus action to enter; ends early if the
    /// barbarian falls unconscious or doesn't attack / take damage on
    /// a round — we just let the timer run for simplicity.
    Raging,
    /// Polymorphed (5e level-4 transmutation, concentration). The target
    /// is transformed into a beast form — we model only the load-bearing
    /// mechanical change: their max HP is replaced by a buff pool (we
    /// approximate with +25 max HP, plus filling current HP to that cap)
    /// while the condition is up. On drop, max HP returns to base. The
    /// transformation also halts spellcasting (we don't enforce that
    /// gate). Cleared on concentration drop.
    Polymorphed,
    /// Globe of Invulnerability (5e level-6 abjuration, concentration).
    /// The holder gains immunity to damaging spells of level 5 or lower.
    /// We approximate by giving the holder a flat damage-resistant buff
    /// (halving incoming damage) — distinct from the `DamageResistant`
    /// condition so the two stack cleanly (Stoneskin + Globe halves
    /// damage twice). Concentration-bound on the caster.
    Globed,
    /// Lifted (5e Telekinesis spell, level 5, concentration). Target is
    /// suspended in the air. Mechanically: zero movement (we add this to
    /// `zeros_movement`), disadvantage on attacks made by the lifted
    /// creature (helpless dangling), and the caster can re-position them
    /// each round. We model only the movement-zero half for simplicity.
    Lifted,
    /// Booming-Blade-marked (5e cantrip rider). The blade caster touched
    /// this target on a hit; if they voluntarily move before the start of
    /// the caster's next turn, they take an extra 1d8 thunder damage. We
    /// track this as a tick-down condition on the target; the
    /// movement-damage rider lives on the `MoveActor` apply path (next
    /// to the Spike Growth rider) and consumes the mark on trigger.
    BoomingBladeMarked,
    /// Crusader's Mantle aura (5e level-3 paladin, concentration). The
    /// holder (an ally of the caster, including the caster themselves)
    /// rolls +1d4 radiant on every weapon attack hit. We layer this on
    /// `resolve_attack` as a small radiant rider, mirroring Hunter's
    /// Mark / Hex but typed Radiant. Dispelled when the caster drops
    /// concentration.
    CrusadersMantled,
    /// Mind-Whipped (5e Tasha's Mind Whip, level-2). On a failed INT save
    /// the target takes 3d6 psychic and loses one of their action /
    /// bonus-action / reaction on their next turn. We model the
    /// reaction-loss half via the existing `NoReaction` rider; the action
    /// / bonus-action loss is folded in by zeroing those resources at the
    /// start of the holder's next turn (we tag this condition and
    /// `reset_for_new_round` consumes it). Cleared on tick.
    MindWhipped,
    /// Time Stopped (5e Time Stop, level-9). The caster is given an extra
    /// Action and Bonus Action immediately on cast — we collapse the
    /// 5e "1d4+1 extra turns" clause into a single burst of action
    /// economy that resolves this turn. The condition is a marker only
    /// (it tags the caster so re-cast is detectable / dispel-prunable);
    /// no per-tick mechanics ride it.
    TimeStopped,
    /// Caged by Forcecage (5e level-7, CHA save). The target is sealed in
    /// an impassable cage of force. Mechanically: zero movement (joins
    /// `zeros_movement`). The 5e cage also blocks teleportation and
    /// passwall — neither is modeled here. Lasts the spell's duration
    /// (1 hour RAW; capped here to a long Rounds timer).
    Caged,
    /// Surrounded by a Crown of Stars (5e level-7 evocation). The caster
    /// has 7 motes of radiant light orbiting them; using a Bonus Action
    /// to fling one at a target deals 4d12 radiant. We collapse the
    /// "7 charges over the duration" RAW into a flat damage-rider buff:
    /// the holder rolls +1d8 radiant on every weapon attack while the
    /// crown is up. Mirrors Crusader's Mantle but is self-only and
    /// concentration-free (a flat Rounds timer suffices).
    CrownOfStars,
    /// Divine Smite primed (5e Paladin feature). The paladin has spent a
    /// spell slot via the Divine Smite bonus action; their next successful
    /// melee weapon hit deals +2d8 radiant damage and consumes this flag.
    /// We model the slot-level scaling at the trigger site (resolve_attack
    /// reads it and rolls the per-slot-level dice). Tick-down timer keeps
    /// the prime from outliving the round if the paladin never connects.
    Smiting,
    /// Channel Divinity: Sacred Weapon active (5e Paladin Oath of Devotion).
    /// The paladin's weapon glows with divine light: attack rolls gain a
    /// flat +CHA-modifier bonus (folded into `condition_attack_bonus`).
    /// Lasts up to 10 rounds (1 minute RAW). Tracked as a condition so
    /// the buff drops cleanly when the timer expires or it's dispelled.
    Sacred,
    /// Compelled Duel (5e Paladin level-1 enchantment, concentration). The
    /// target is locked into combat with the caster: they have disadvantage
    /// on attacks against anyone *other* than the caster, and must save
    /// against the spell to move further than 30ft from them. We model only
    /// the load-bearing half: the disadvantage on attacks against non-caster
    /// targets (read by `compute_attack_mode`). Tracked as a condition with
    /// a `dueled_by` link so the engine knows who the duel is anchored on.
    Dueled,
    /// Searing Smite primed (5e level-1 paladin evocation, bonus action).
    /// The paladin's weapon erupts in fire on the primed hit: +1d6 fire
    /// rider via the on-hit rider table, plus the target catches fire
    /// (Burning, 3 rounds) on the same swing. One-shot prime — consumed
    /// the moment the rider lands.
    SearingSmiting,
    /// Wrathful Smite primed (5e level-1 paladin enchantment, bonus
    /// action). +1d6 psychic rider on the primed hit, then a WIS save
    /// (caster's CHA-based DC) gates a 10-round Frightened on the
    /// target on fail. One-shot.
    WrathfulSmiting,
    /// Branding Smite primed (5e level-2 paladin evocation, bonus
    /// action). +2d6 radiant rider on the primed hit; target also lights
    /// up (Outlined for 10 rounds — attackers get advantage and any
    /// concurrent Invisibility / Hidden status falls off via the
    /// existing Outlined hooks). Auto-apply on hit (no save in RAW).
    BrandingSmiting,
    /// Blinding Smite primed (5e level-3 paladin evocation, bonus
    /// action). +3d8 radiant rider on the primed hit; target makes a CON
    /// save vs the caster's CHA-based DC or is Blinded for 10 rounds.
    /// One-shot.
    BlindingSmiting,
    /// Heat Metal (5e level-2 transmutation, concentration). The target's
    /// metal armor / weapon glows red-hot: 2d8 fire on cast and on each
    /// of the holder's turn-start ticks while concentration holds. They
    /// also have disadvantage on attacks and ability checks (we read the
    /// Mocked-style disadvantage clause via `compute_attack_mode`'s
    /// extension). Cleared when the caster drops concentration.
    HeatMetaled,
    /// Stunning Strike pending (5e Monk feature). On the next melee hit,
    /// the monk spends a ki point and the target makes a CON save vs the
    /// monk's WIS-based DC (8 + prof + WIS); on fail, the target is
    /// Stunned until the end of the monk's next turn. We model this as a
    /// caster-side prime (similar to Divine Smite) — the on-hit hook
    /// reads the flag, queues the save, and clears the prime. One-shot.
    StunningStrike,
    /// Bardic Inspiration die granted (5e Bard feature). The holder may
    /// add a `bardic_inspiration_die` (1d6 by default in this engine) to
    /// the next attack roll, save, or ability check they make. We honor
    /// the attack-roll bump via `condition_attack_bonus` (flat +3, the
    /// d6-average rounded down) and clear the condition on consume.
    /// Concentration-free; the timer caps unused inspiration at 10
    /// rounds (1 minute RAW).
    Inspired,
    /// Exhausted (5e Exhaustion, simplified to a single level). RAW
    /// models 6 cumulative tiers; we collapse to one flag with the
    /// load-bearing penalties: disadvantage on attack rolls AND ability
    /// checks (tier 1) plus disadvantage on saving throws (tier 3).
    /// Cleared by a long rest. Distinct from `Poisoned` so cleanse
    /// pickers (Lesser Restoration / Greater Restoration) can target
    /// it explicitly.
    Exhausted,
    /// Spirit Shroud (5e level-3, concentration). The holder wraps
    /// themselves in deathly mist: every melee weapon attack the holder
    /// lands deals an extra 1d8 cold damage, and the target's speed is
    /// reduced (we model only the on-hit rider half — cold typed per
    /// RAW's default flavor). Mirrors Crusader's Mantle / Crown of Stars
    /// in the OnHitRider table — persistent (non-consumed) and non-
    /// melee-only since our weapon-vs-spell-attack split surfaces the
    /// melee filter centrally.
    SpiritShrouded,
    /// Holy Aura (5e level-8 abjuration, concentration). The holder and
    /// every ally inside a 30ft sphere benefit from advantage on all
    /// saving throws, and attackers against them have disadvantage on
    /// attack rolls. We model the load-bearing half via two condition
    /// hooks: `compute_save_mode` reads the flag for advantage, and
    /// `compute_attack_mode` reads it on the target for the
    /// attacker-disadvantage clause. Concentration-bound on the caster.
    HolyAuraed,
    /// Foresight (5e level-9 divination, concentration). The target has
    /// advantage on every attack roll, saving throw, and ability check,
    /// and attackers against them have disadvantage. The mightiest
    /// single-target buff in the SRD — we wire the attack-advantage
    /// half through `compute_attack_mode` on the attacker side, the
    /// save-advantage half through `compute_save_mode`, and the
    /// disadvantage-to-attackers half through `compute_attack_mode` on
    /// the target side. Concentration-bound on the caster.
    Foreseen,
    /// Confused (5e Confusion spell, level-4 enchantment, concentration).
    /// The target's mind is scrambled: in RAW they roll on a chaos table
    /// each turn (attack random ally, babble, do nothing). We collapse the
    /// table into the load-bearing penalty: disadvantage on attack rolls
    /// (joins the `Frightened`/`Poisoned` cohort in `compute_attack_mode`)
    /// AND the target cannot take Reactions (joins the `NoReaction` cohort
    /// in the action-economy gate). Concentration-bound on the caster;
    /// applied on a failed WIS save when the spell's burst lands.
    Confused,
    /// Entangled (5e Plant Growth spell, level-3 transmutation). The area
    /// erupts with grasping vines: every creature in the 20ft burst is
    /// snared in place for the duration. Mechanically: movement is zeroed
    /// (joins `zeros_movement`) but the target can still act (unlike
    /// `Restrained`, no attack-roll penalty / DEX-save disadvantage). The
    /// spell has no concentration; we apply with a Rounds(10) timer and
    /// let it tick down. Doesn't stack with Restrained — both flags
    /// together still just zero movement once.
    Entangled,
    /// Flying (5e Fly spell, level-3 transmutation, concentration). The
    /// target gains a flying speed bonus — read by `speed()` as +24 tiles
    /// (60ft RAW). We don't model 3D positioning; the bonus represents the
    /// raw kiting advantage. Concentration-bound on the caster; the spell
    /// drops the buff when concentration ends. Joins `is_dispellable_buff`
    /// so Dispel Magic / Counterspell can rip it.
    Flying,
    /// Dominated (5e Dominate Person, level-5 enchantment, concentration).
    /// The target's will is overridden by the caster. We model the
    /// load-bearing half: the target is Charmed by the caster (so they
    /// can't attack them, via the existing `charmed_by` block) AND any
    /// attack the target makes against anyone other than the caster's
    /// enemies is at disadvantage — we approximate by giving the holder
    /// blanket attack disadvantage (they hesitate, fight the compulsion).
    /// Concentration-bound; broken when the dominator drops concentration
    /// or the target makes a successful WIS save (which we re-trigger via
    /// damage in 5e; we keep the spell duration-bound for simplicity).
    Dominated,
    /// Feebled (5e Feeblemind, level-8 enchantment). The target's INT
    /// and CHA scores effectively drop to 1: they can't focus, can't
    /// cast spells, can't sustain attention. We model the load-bearing
    /// penalties:
    /// - Disadvantage on attack rolls (joins the `imposes_attacker_
    ///   disadvantage` cohort) — the target swings dazed.
    /// - Disadvantage on INT, WIS, and CHA saving throws (read by
    ///   `compute_save_mode`'s Feebled clause).
    /// - The "can't cast spells" RAW clause is *not* enforced (the
    ///   engine has no spell-component gate), but Greater Restoration
    ///   explicitly removes Feebled so the cleanse path stays intact.
    ///
    /// Cleared by a Greater Restoration cleanse or by long rest.
    Feebled,
    /// Dancing (5e Otto's Irresistible Dance, level-6 enchantment,
    /// concentration). The target capers helplessly: they have disadvantage
    /// on attacks (joins the `imposes_attacker_disadvantage` cohort);
    /// attacks against them have advantage (joins
    /// `grants_advantage_to_attackers`); they auto-fail DEX saves; movement
    /// is zero (joins `zeros_movement`). The dance lasts up to 10 rounds —
    /// RAW lets the target spend an Action to attempt a WIS save each turn;
    /// we collapse to the duration-bound install for simplicity.
    /// Concentration-bound on the caster.
    Dancing,
    /// Mazed (5e Maze, level-8 conjuration, concentration). The target is
    /// banished to a demiplane: they're effectively removed from the
    /// encounter map for the spell's duration. Mechanically we treat the
    /// Maze condition as a full incapacitation envelope — zero movement
    /// (joins `zeros_movement`), all action economy blocked
    /// (`blocks_action_economy`), and reactions blocked. We don't
    /// physically delete the actor; the engine just makes them inert.
    /// RAW gives the target an INT check at the end of each of their turns
    /// to escape; we leave the duration-bound install for simplicity.
    /// Concentration-bound on the caster.
    Mazed,
    /// Eyebitten — Asleep (5e Eyebite spell, level-6 necromancy,
    /// concentration). The target falls unconscious; mechanically we route
    /// through the existing `Asleep` condition for the action-economy
    /// envelope. This variant exists so the eyebite-specific log line and
    /// the concentration mark have a distinct condition to track, and so
    /// dropping concentration cleans up the eyebite mark cleanly.
    /// Currently inert beyond marking — the load-bearing penalties land on
    /// the `Asleep` rider applied alongside.
    EyebittenSick,
    /// Conjured (5e Conjure Animals and related). The summoned minion
    /// holds this flag so the caster's concentration drop can prune the
    /// minion (dispel the conjured creature) cleanly via the
    /// `dispel_conjured_on_concentration_drop` hook. Inert otherwise; the
    /// summoned actor behaves like any team-mate while the flag is up.
    Conjured,
    /// Armor of Agathys (5e Warlock level-1 abjuration). The holder is
    /// wreathed in protective freezing ice. They gain 5 temp HP (the
    /// `GainTempHp` install lane handles the buffer) and any creature that
    /// hits them with a melee attack takes 5 cold damage. The retaliation
    /// rider lives on `EncounterInstance::resolve_attack` next to the
    /// Fire Shield reflect — mirrors the same shape, with cold damage and
    /// a melee-only filter. Drops when the temp HP pool hits zero (we
    /// strip the condition the moment temp HP is exhausted, since the
    /// shield is the temp HP) OR after the spell timer runs out.
    /// Concentration-free; a flat Rounds timer is enough.
    AgathysShielded,
    /// Sickening Radiance (5e level-4 evocation, concentration). The
    /// target sits inside a 30ft sphere of radiant light: every round
    /// they're in the zone, they take 4d10 radiant on a failed CON save
    /// and gain a level of exhaustion. We collapse the "sustained zone"
    /// to a one-shot install at cast time — the burst rolls saves up
    /// front, applies damage, and installs Exhausted on every actor
    /// that failed. The condition tag itself is a marker so the
    /// concentration cleanup hook can find it.
    SickeningRadiated,
    /// Bigby's Hand (5e level-5 evocation, concentration). The caster
    /// summons a spectral hand of force that follows enemies around
    /// pounding them. We collapse the spell's many activation modes
    /// (grasping, slamming, interposing) into a flat +1d10 force-typed
    /// per-hit rider on the caster's weapon attacks — slots into the
    /// existing OnHitRider table next to Crown of Stars / Crusader's
    /// Mantle. Concentration-bound so re-casting cleans up.
    BigbysHanded,
    /// Tenser's Transformation (5e level-6 transmutation, concentration).
    /// The caster becomes a battle-trance avatar — they gain advantage
    /// on weapon attacks (joins `grants_self_attack_advantage`), and
    /// the spell hands out 50 temp HP on cast. We skip the RAW
    /// "proficient with all weapons / +2d12 force on weapon hits"
    /// clauses since those would need bookkeeping for weapon damage
    /// types; the headline self-advantage + temp HP envelope is the
    /// load-bearing buff. Concentration-bound on the caster.
    Transformed,
    /// Divine Strike primed (5e Cleric Channel Divinity flavor; we model
    /// the level-8-and-above feature as a once-per-rest prime that lands
    /// on the caster's next melee hit for +1d8 radiant damage. Mirrors
    /// the Smiting / SearingSmiting one-shot prime pattern. Cleared by
    /// the OnHitRider table the moment the rider lands. Tick-down timer
    /// keeps a swing-less prime from dangling indefinitely.
    DivineStriking,
    /// Trip Attack primed (5e Fighter Battle Master maneuver). The next
    /// melee weapon hit forces the target to make a STR save vs the
    /// fighter's maneuver DC (8 + prof + STR); on fail, the target is
    /// knocked Prone. One-shot — the rider table strips this flag the
    /// moment it lands. Tick-down timer caps the prime so an idle
    /// fighter doesn't carry the maneuver across rests.
    TripAttacking,
    /// Invested with Flame (5e level-6 transmutation, concentration). The
    /// caster's body burns with elemental fire: they gain resistance to
    /// fire damage (via the generic `DamageResistant` model) and any
    /// creature within reach that hits them with a melee attack takes
    /// 1d10 fire damage in retaliation. Shape mirrors `FireShielded` but
    /// the spell is concentration-bound on the caster and self-only.
    /// Cleared on concentration drop.
    InvestedInFlame,
    /// Mental Prison — RAW: imprisoned for the duration in an illusion
    /// of agony. We model the load-bearing half as a Restrained envelope
    /// (movement zero, attacks with disadvantage, attacks against have
    /// advantage). Concentration-bound on the caster; the install rider
    /// also bursts 5d10 psychic on cast. Distinct from `Restrained` so
    /// the concentration cleanup can drop just this mark cleanly.
    MentallyImprisoned,
    /// Otiluke's Resilient Sphere (5e level-4 evocation, concentration).
    /// The target is encased in an indestructible sphere of force.
    /// Mechanically: zero movement, the target can't take actions or
    /// reactions that require leaving the sphere — we collapse to a
    /// `blocks_action_economy` envelope. Attacks against them have
    /// advantage (the sphere immobilizes a flailing target) and DEX saves
    /// are at disadvantage. Concentration-bound on the caster; dropping
    /// concentration shatters the sphere cleanly.
    Sphered,
    /// Wind Wall (5e level-3 evocation, concentration). The holder stands
    /// behind a vertical wall of strong wind: ranged attacks against them
    /// have disadvantage (RAW: arrows / bolts deflect, breath weapons and
    /// gases dissipate). Melee swings are unaffected. Concentration-bound
    /// on the caster — dropping concentration ends the wall. Distinct
    /// from `Blurred` / `Foreseen` so a wind-walled actor stacks cleanly
    /// with a separate visual buff.
    WindWalled,
    /// Staggering Smite primed (5e level-4 paladin enchantment, bonus
    /// action). +4d6 psychic rider on the primed hit; target makes a WIS
    /// save vs the caster's CHA-based DC or is Stunned until the end of
    /// the paladin's next turn. One-shot — consumed when the rider lands.
    /// Tick-down timer keeps a swing-less prime from dangling indefinitely.
    StaggeringSmiting,
    /// Banishing Smite primed (5e level-5 paladin abjuration, bonus
    /// action). +5d10 force rider on the primed hit; if the rider reduces
    /// the target to HP <= 50, the target is also Banished (we collapse
    /// the demi-plane mechanic to a 10-round inert envelope via the
    /// existing `Mazed` condition — same end-state, distinct log line).
    /// One-shot — the rider table strips this flag the moment it lands.
    BanishingSmiting,
    /// Thunderous Smite primed (5e level-1 paladin evocation, bonus
    /// action). +2d6 thunder rider on the primed hit; target makes a STR
    /// save vs the paladin's CHA-based DC or is pushed 10 ft (4 tiles)
    /// away and knocked Prone. We collapse the RAW "push 10 ft + prone"
    /// rider to a Prone follow-up via the smite follow-up site, since the
    /// push routes through the standard `PushActor` helper and the
    /// rider's load-bearing crowd-control effect is the prone tag.
    /// One-shot — the rider table strips this flag the moment it lands.
    ThunderousSmiting,
    /// Shillelagh primed (5e cantrip, druid). The caster's club / staff /
    /// quarterstaff is imbued with sylvan magic: the next melee weapon
    /// hit deals an extra 1d8 force damage and the swing rolls vs the
    /// caster's WIS modifier instead of STR (we collapse the swing-stat
    /// swap since the rider damage is the load-bearing portion). One-shot
    /// — the rider table strips this flag the moment it lands. Tick-down
    /// timer caps the prime so a swing-less druid doesn't carry it across
    /// rests. Concentration-free per RAW (the spell has a 1-minute
    /// duration, not concentration).
    Shillelaghed,
    /// Grasped by Maximilian's Earthen Grasp (5e level-2 transmutation,
    /// concentration). The target is held in a fist of magical earth:
    /// Restrained envelope (zero movement, attack disadvantage, attacks
    /// against have advantage) plus a 2d6 bludgeoning DoT at the end of
    /// every round the spell holds. Concentration-bound on the caster;
    /// dropping concentration releases the grip. Distinct from `Grappled`
    /// / `Restrained` so the log line and concentration cleanup target
    /// just this mark.
    EarthenGrasped,
    /// Coated in residual acid from Vitriolic Sphere (5e level-4
    /// evocation). On a failed save, the target gets a one-tick acid
    /// DoT: 5d4 acid at the next round-end, then the condition expires.
    /// Distinct from the immediate damage on cast — the spell rolls
    /// 10d4 acid immediately and queues this mark for the second-round
    /// drip. Self-clears via `Rounds(1)` timer.
    VitriolicAcidCoated,
    /// Enlarged by the Enlarge / Reduce spell (5e level-2 transmutation,
    /// concentration; the Reduce twin is the symmetric debuff and isn't
    /// modeled separately here). The target's size category bumps up by
    /// one and they roll +1d4 extra damage on weapon attacks (read by the
    /// on-hit rider table). RAW also grants advantage on STR checks and
    /// STR saves — we surface only the load-bearing damage rider since the
    /// engine's check / save lanes don't have a per-stat advantage hook
    /// that other buffs use. Concentration-bound on the caster; dropping
    /// concentration drops the buff.
    Enlarged,
    /// Bonded by Warding Bond (5e level-2 abjuration). The bonded actor
    /// gains +1 AC, +1 saving throws, and resistance to all damage. Any
    /// damage that lands on the bonded actor is mirrored onto their
    /// bonding partner (the caster) at the post-resistance amount; the
    /// `warding_partner` link on `ActorInstance` carries the partner id so
    /// the reflect site can find the caster. Distinct from `DamageResistant`
    /// so dispel / drop-on-distance can target just this mark. RAW: 1-hour
    /// duration, no concentration; we install with a long Rounds(60) timer
    /// (~10 minutes of combat — long enough for any encounter, short enough
    /// that the link isn't permanent across rests). The bond ends RAW when
    /// either creature drops to 0 HP — we keep the simple "timer or dispel"
    /// drop path for now (the partner's chain damage takes care of the
    /// caster naturally on a fatal mirror hit).
    WardingBonded,
    /// Mind Blanked (5e level-8 abjuration). The target's mind is sealed:
    /// they're immune to psychic damage (zeroed in `effective_damage`) and
    /// to the Charmed condition (gated in `add_condition`). RAW: lasts
    /// 24 hours, no concentration; we install with a long Rounds(100)
    /// timer so it covers any plausible encounter span without becoming
    /// truly permanent. Joins `is_dispellable_buff` so the spell can be
    /// stripped by Dispel Magic's beneficial-buff fallback.
    MindBlanked,
    /// Lightning Arrow primed (5e level-3 ranger evocation, concentration).
    /// The ranger's next ranged weapon attack hit deals +4d8 lightning
    /// via the on-hit rider table. One-shot prime — the rider table
    /// strips this flag the moment a ranged hit consumes it. Mirrors the
    /// Smite-spell prime pattern; the new `ranged_only` flag on
    /// OnHitRider gates the rider to bow swings so a melee fallback
    /// can't burn the prime.
    LightningArrowPrimed,
    /// Barkskin (5e level-2 transmutation, concentration). The target's
    /// skin hardens to bark: their AC becomes 16 unless their natural /
    /// worn-armor AC is already higher. We model the "AC floor" via the
    /// `barkskin_floor()` accessor on `ActorInstance`, mirroring how
    /// `MageArmored` plugs into `armor_class()`. Concentration-bound on
    /// the caster; the buff drops cleanly when concentration ends.
    Barkskinned,
    /// Pass Without Trace (5e level-2 abjuration, concentration). The
    /// holder steps lightly through the world — attackers have
    /// disadvantage on attack rolls against them (RAW gives +10 to
    /// Stealth checks; we collapse the resulting harder-to-target effect
    /// into the disadvantage-on-attackers cohort). Concentration-bound
    /// on the caster; applied to every ally inside the 30ft aura at
    /// cast time. Joins `is_dispellable_buff` so Dispel Magic can rip
    /// the cover.
    Untracked,
    /// Holy Weapon (5e level-5 paladin evocation, concentration). The
    /// holder's weapon is sheathed in radiant light: every weapon hit
    /// deals an extra 2d8 radiant damage. We model the per-hit rider
    /// via the on_hit_riders table next to Crusader's Mantle / Spirit
    /// Shroud — persistent (not consumed on trigger) and self-only.
    /// Concentration-bound on the caster.
    HolyWeaponed,
    /// Displaced (5e Displacer Beast trait). The holder's outline shifts
    /// and wavers: attacks against them have disadvantage. Broken the
    /// first time the holder takes damage — once hit, the beast's
    /// displacement flickers off until the end of its next turn.
    /// Modeled as a condition that imposes disadvantage to attackers
    /// (joins `imposes_disadvantage_to_attackers`) and self-restores at
    /// the start of the holder's turn.
    Displaced,
    /// Absorb Elements (5e level-1 abjuration, reaction). The holder
    /// has captured incoming elemental energy: they gain resistance to
    /// the triggering damage type for the rest of the round, and their
    /// next melee attack deals +1d6 of the absorbed element. We model
    /// the load-bearing resistance half as a one-round `DamageResistant`
    /// style flag. The melee rider is handled through the on_hit_riders
    /// table. Self-clears via `UntilStartOfNextTurn`.
    AbsorbedElements,
    /// Danger Sense active (5e Barbarian level 2). The holder has
    /// advantage on DEX saving throws against effects they can see
    /// (traps, spells). We model as a permanent passive: always-on
    /// advantage on DEX saves while not Blinded / Incapacitated /
    /// Deafened. Read by `compute_save_mode`'s DEX-advantage clause.
    DangerSense,
    /// Witch Bolt tethered (5e level-1 evocation, concentration). The
    /// caster maintains a lightning tether to the target: at the start of
    /// each of the caster's turns, the target takes 1d12 lightning
    /// automatically (no attack roll, no save). Concentration-bound; the
    /// bolt ends when concentration drops or the target moves out of
    /// range. We model as a DoT condition on the target with the standard
    /// ROUND_END_DOTS drip so the damage ticks uniformly.
    WitchBolted,
    /// Spirit Guardians (5e level-3 conjuration, concentration). The
    /// caster is surrounded by spectral warriors: every hostile creature
    /// that starts its turn within 15ft (6 tiles) takes 3d8 radiant
    /// damage on a failed WIS save (half on pass). We model the
    /// load-bearing half as a condition on the caster that triggers an
    /// aura-style DoT at round-end for each nearby enemy.
    SpiritGuarding,
    /// Moonbeam (5e level-2 evocation, concentration). A 5ft-radius
    /// cylinder of pale light shines down: creatures entering or starting
    /// their turn in the area make a CON save or take 2d10 radiant (half
    /// on pass). Shapechangers auto-fail. We collapse the zone to a
    /// condition on targets caught in the initial burst, with a round-end
    /// DoT drip for the sustained damage.
    Moonbeamed,
    /// Cloud of Daggers (5e level-2 conjuration, concentration). A 5ft
    /// cube of spinning daggers fills the area: any creature that enters
    /// or starts its turn there takes 4d4 slashing automatically (no
    /// save). We collapse to a condition-tagged DoT on targets caught in
    /// the initial placement.
    CloudOfDaggered,
    /// Counterspelled — marker placed briefly during the counter-magic
    /// resolution. Not a real debuff; used by the engine to track that
    /// a spell was counterspelled this stack frame. Inert otherwise.
    Counterspelled,
}

impl Condition {
    pub fn name(&self) -> &'static str {
        match self {
            Condition::Prone => "prone",
            Condition::Stunned => "stunned",
            Condition::Poisoned => "poisoned",
            Condition::Frightened => "frightened",
            Condition::Restrained => "restrained",
            Condition::Blinded => "blinded",
            Condition::Incapacitated => "incapacitated",
            Condition::Charmed => "charmed",
            Condition::Paralyzed => "paralyzed",
            Condition::Invisible => "invisible",
            Condition::Dodging => "dodging",
            Condition::Grappled => "grappled",
            Condition::Blessed => "blessed",
            Condition::ShieldOfFaith => "shield of faith",
            Condition::Shielded => "shielded",
            Condition::DamageResistant => "damage resistant",
            Condition::Outlined => "outlined",
            Condition::GuidingBoltLit => "marked by guiding bolt",
            Condition::Helped => "helped",
            Condition::Hidden => "hidden",
            Condition::Burning => "burning",
            Condition::Disengaging => "disengaging",
            Condition::Unconscious => "unconscious",
            Condition::Baned => "baned",
            Condition::Deafened => "deafened",
            Condition::HuntersMarked => "marked by hunter's mark",
            Condition::MageArmored => "mage armored",
            Condition::Mocked => "mocked",
            Condition::Heroic => "heroic",
            Condition::NoReaction => "shocked",
            Condition::Asleep => "asleep",
            Condition::MirroredImages => "mirror imaged",
            Condition::Warded => "warded",
            Condition::Hexed => "hexed",
            Condition::Blurred => "blurred",
            Condition::Adhered => "stuck",
            Condition::Hasted => "hasted",
            Condition::Slowed => "slowed",
            Condition::DeathWarded => "warded against death",
            Condition::Petrified => "petrified",
            Condition::Sanctuary => "sanctified",
            Condition::FireShielded => "fire shielded",
            Condition::Daylit => "lit by daylight",
            Condition::Spiked => "in spiked growth",
            Condition::Raging => "raging",
            Condition::Polymorphed => "polymorphed",
            Condition::Globed => "globed in invulnerability",
            Condition::Lifted => "lifted by telekinesis",
            Condition::BoomingBladeMarked => "thunder-marked",
            Condition::CrusadersMantled => "crusader's mantled",
            Condition::MindWhipped => "mind-whipped",
            Condition::TimeStopped => "time-stopped",
            Condition::Caged => "caged in force",
            Condition::CrownOfStars => "haloed by stars",
            Condition::Smiting => "smiting",
            Condition::Sacred => "wielding a sacred weapon",
            Condition::Dueled => "compelled to duel",
            Condition::SearingSmiting => "primed to sear",
            Condition::WrathfulSmiting => "primed with wrath",
            Condition::BrandingSmiting => "primed to brand",
            Condition::BlindingSmiting => "primed to blind",
            Condition::HeatMetaled => "burning from heat metal",
            Condition::StunningStrike => "primed to stun",
            Condition::Inspired => "inspired",
            Condition::Exhausted => "exhausted",
            Condition::SpiritShrouded => "wreathed in spirits",
            Condition::HolyAuraed => "haloed in holy light",
            Condition::Foreseen => "foreseen",
            Condition::Confused => "confused",
            Condition::Entangled => "entangled",
            Condition::Flying => "flying",
            Condition::Dominated => "dominated",
            Condition::Feebled => "feebleminded",
            Condition::Dancing => "dancing helplessly",
            Condition::Mazed => "trapped in a maze",
            Condition::EyebittenSick => "afflicted by eyebite",
            Condition::Conjured => "conjured",
            Condition::AgathysShielded => "armored in agathys",
            Condition::SickeningRadiated => "sickened with radiance",
            Condition::BigbysHanded => "guarded by bigby's hand",
            Condition::Transformed => "transformed",
            Condition::DivineStriking => "primed with divine strike",
            Condition::TripAttacking => "primed to trip",
            Condition::InvestedInFlame => "invested with flame",
            Condition::MentallyImprisoned => "mentally imprisoned",
            Condition::Sphered => "trapped in a resilient sphere",
            Condition::WindWalled => "sheltered by a wind wall",
            Condition::StaggeringSmiting => "primed to stagger",
            Condition::BanishingSmiting => "primed to banish",
            Condition::ThunderousSmiting => "primed with thunder",
            Condition::Shillelaghed => "wielding a shillelagh",
            Condition::EarthenGrasped => "crushed by an earthen grasp",
            Condition::VitriolicAcidCoated => "coated in vitriolic acid",
            Condition::Enlarged => "enlarged",
            Condition::WardingBonded => "bonded by warding bond",
            Condition::MindBlanked => "mind-blanked",
            Condition::LightningArrowPrimed => "primed with lightning arrow",
            Condition::Barkskinned => "barkskinned",
            Condition::Untracked => "passing without trace",
            Condition::HolyWeaponed => "wielding a holy weapon",
            Condition::Displaced => "displaced",
            Condition::AbsorbedElements => "absorbing elements",
            Condition::DangerSense => "sensing danger",
            Condition::WitchBolted => "tethered by witch bolt",
            Condition::SpiritGuarding => "guarded by spirits",
            Condition::Moonbeamed => "caught in moonbeam",
            Condition::CloudOfDaggered => "shredded by daggers",
            Condition::Counterspelled => "counterspelled",
        }
    }

    /// True if this condition completely blocks Action / BonusAction /
    /// Reaction usage (5e's Incapacitated clause). Stunned and Paralyzed
    /// inherit this clause.
    pub fn blocks_action_economy(&self) -> bool {
        matches!(
            self,
            Condition::Stunned
                | Condition::Incapacitated
                | Condition::Paralyzed
                | Condition::Unconscious
                | Condition::Asleep
                | Condition::Petrified
                | Condition::Mazed
                | Condition::Sphered
        )
    }

    /// True if this condition is a beneficial buff that Dispel Magic /
    /// similar "end one effect" spells should target. Used by the
    /// Dispel Magic side-effect when the target isn't concentrating —
    /// the spell falls back to stripping one helpful condition rather
    /// than failing silently.
    pub fn is_dispellable_buff(&self) -> bool {
        matches!(
            self,
            Condition::Blessed
                | Condition::ShieldOfFaith
                | Condition::MageArmored
                | Condition::Heroic
                | Condition::Hasted
                | Condition::Hidden
                | Condition::Invisible
                | Condition::DamageResistant
                | Condition::MirroredImages
                | Condition::Blurred
                | Condition::DeathWarded
                | Condition::Helped
                | Condition::Sanctuary
                | Condition::FireShielded
                | Condition::Daylit
                | Condition::Raging
                | Condition::Polymorphed
                | Condition::Globed
                | Condition::CrusadersMantled
                | Condition::CrownOfStars
                | Condition::TimeStopped
                | Condition::Smiting
                | Condition::Sacred
                | Condition::SearingSmiting
                | Condition::WrathfulSmiting
                | Condition::BrandingSmiting
                | Condition::BlindingSmiting
                | Condition::Inspired
                | Condition::SpiritShrouded
                | Condition::HolyAuraed
                | Condition::Foreseen
                | Condition::Flying
                | Condition::AgathysShielded
                | Condition::BigbysHanded
                | Condition::Transformed
                | Condition::DivineStriking
                | Condition::TripAttacking
                | Condition::InvestedInFlame
                | Condition::WindWalled
                | Condition::StaggeringSmiting
                | Condition::BanishingSmiting
                | Condition::ThunderousSmiting
                | Condition::Shillelaghed
                | Condition::Enlarged
                | Condition::WardingBonded
                | Condition::MindBlanked
                | Condition::LightningArrowPrimed
                | Condition::Barkskinned
                | Condition::Untracked
                | Condition::HolyWeaponed
                | Condition::Displaced
                | Condition::AbsorbedElements
                | Condition::SpiritGuarding
        )
    }

    /// True if this condition zeros out movement. Read by
    /// `ActorInstance::remaining_movement` to gate motion-blocking
    /// conditions in one place.
    pub fn zeros_movement(&self) -> bool {
        // Note: Prone is NOT in this list. RAW: prone halves movement
        // (you crawl). We don't yet model the half-speed reduction;
        // movement just costs the same. But blocking it entirely creates
        // a catch-22 — standing up pays in movement.
        matches!(
            self,
            Condition::Stunned
                | Condition::Restrained
                | Condition::Grappled
                | Condition::Paralyzed
                | Condition::Unconscious
                | Condition::Asleep
                | Condition::Adhered
                | Condition::Petrified
                | Condition::Lifted
                | Condition::Caged
                | Condition::Entangled
                | Condition::Dancing
                | Condition::Mazed
                | Condition::MentallyImprisoned
                | Condition::Sphered
                | Condition::EarthenGrasped
        )
    }

    /// True if this condition imposes disadvantage on every attack roll
    /// the holder makes. Centralized so the `compute_attack_mode` list and
    /// any future "is this attacker debuffed?" sites read from one table.
    /// Used by the engine's attack-mode resolver and surfaced for AI
    /// heuristics (e.g. "is this caster's swing penalized?").
    pub fn imposes_attacker_disadvantage(&self) -> bool {
        matches!(
            self,
            Condition::Prone
                | Condition::Poisoned
                | Condition::Frightened
                | Condition::Restrained
                | Condition::Blinded
                | Condition::Mocked
                | Condition::HeatMetaled
                | Condition::Exhausted
                | Condition::Confused
                | Condition::Dominated
                | Condition::Feebled
                | Condition::Dancing
                | Condition::MentallyImprisoned
                | Condition::Sphered
                | Condition::EarthenGrasped
        )
    }

    /// True if this condition grants the holder advantage on their own
    /// attack rolls. Centralized for the same reason as
    /// `imposes_attacker_disadvantage` — one list, one source of truth.
    pub fn grants_self_attack_advantage(&self) -> bool {
        matches!(
            self,
            Condition::Invisible
                | Condition::Helped
                | Condition::Hidden
                | Condition::Foreseen
                | Condition::Blessed
                | Condition::Transformed
        )
    }

    /// True if attacks targeting the holder get advantage. Mirrors the
    /// "target-side advantage" cohort of `compute_attack_mode`. The
    /// `Prone`-melee-only edge case is handled at the call site (it's not
    /// universally advantage — ranged attackers get disadvantage instead).
    pub fn grants_advantage_to_attackers(&self) -> bool {
        matches!(
            self,
            Condition::Stunned
                | Condition::Restrained
                | Condition::Blinded
                | Condition::Incapacitated
                | Condition::Paralyzed
                | Condition::Unconscious
                | Condition::Outlined
                | Condition::Petrified
                | Condition::GuidingBoltLit
                | Condition::Dancing
                | Condition::MentallyImprisoned
                | Condition::Sphered
                | Condition::EarthenGrasped
        )
    }

    /// True if attacks targeting the holder get disadvantage. Mirrors the
    /// "target-side disadvantage" cohort of `compute_attack_mode`.
    /// `Warded` / `Daylit` are *not* in this list — they impose
    /// disadvantage only against undead / fiend attackers, which needs
    /// attacker-side state to evaluate; the call site handles them.
    /// `WindWalled` is *not* in this list — it imposes disadvantage only
    /// on ranged attacks (see `imposes_disadvantage_to_ranged_attackers`).
    pub fn imposes_disadvantage_to_attackers(&self) -> bool {
        matches!(
            self,
            Condition::Invisible
                | Condition::Dodging
                | Condition::Blurred
                | Condition::HolyAuraed
                | Condition::Foreseen
                | Condition::Untracked
                | Condition::Displaced
        )
    }

    /// True if *ranged* attacks targeting the holder get disadvantage but
    /// melee attacks are unaffected. The 5e Wind Wall clause is the
    /// canonical case — arrows / bolts deflect, swords don't. Read by
    /// `compute_attack_mode` only when `!is_melee` so a wind-walled
    /// caster still eats melee damage normally.
    pub fn imposes_disadvantage_to_ranged_attackers(&self) -> bool {
        matches!(self, Condition::WindWalled)
    }

    /// True if the holder cannot take Reactions while this condition is
    /// up. Covers the explicit `NoReaction` lockout and the new `Confused`
    /// clause (RAW: chaos table prevents reactions). Read by
    /// `can_consume_resource` alongside the `blocks_action_economy` cohort
    /// so the gate has one chokepoint per resource lane.
    pub fn blocks_reactions(&self) -> bool {
        matches!(
            self,
            Condition::NoReaction | Condition::Confused | Condition::Sphered
        )
    }
}

impl std::fmt::Display for Condition {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// How long a condition application persists. `Permanent` requires an
/// explicit removal (e.g. Stand-up clears Prone, Lesser Restoration clears
/// Poisoned). `Rounds(n)` ticks down by 1 every time the initiative queue
/// wraps; the condition is removed when the timer hits 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionTimer {
    Permanent,
    Rounds(u32),
    /// Lasts until the start of the holder's next turn — used for Dodge.
    /// The engine clears these at the start of an actor's turn.
    UntilStartOfNextTurn,
}

