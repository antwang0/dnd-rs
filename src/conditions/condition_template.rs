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
    /// 5e Charmed: "A charmed creature can't attack the charmer or
    /// target the charmer with harmful abilities or magic effects."
    /// Enforced — the restriction rides on the `Charmed` back-link the
    /// installing spell / monster attack sets alongside the condition,
    /// and is read through `EncounterInstance::charm_blocks_hostility`
    /// at all three lanes that can reach hostility: declared actions
    /// (`Action::validate`, across every target in the list),
    /// opportunity attacks, and Riposte. Purely defensive reactions
    /// (Uncanny Dodge, Deflect Missiles, Parry, Warding Flare) stay
    /// available against the charmer — RAW forbids attacking them, not
    /// surviving them.
    ///
    /// RAW's second clause — the charmer's advantage on social ability
    /// checks — has no combat surface here and is not modeled.
    ///
    /// The `Charmed` back-link is cleared when the condition is removed,
    /// so a charm that lapses mid-fight restores hostility on the same
    /// tick rather than leaving a stale immunity behind.
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
    /// Holding a readied attack (5e **Ready**). The holder has spent
    /// their Action to say "I attack the first enemy that comes into
    /// range", and the swing fires as a reaction the moment one does —
    /// see `EncounterInstance::dispatch_readied_attacks`.
    ///
    /// Installed with an `UntilStartOfNextTurn` timer, which is exactly
    /// RAW's window: "you can act at any time up to the start of your
    /// next turn". A readier whose trigger never happens simply loses
    /// the action, the same as at a table.
    ///
    /// What it *doesn't* model is the choice. RAW lets you ready any
    /// action against any trigger you can describe; the readier here
    /// holds one attack against one trigger — "an enemy comes within
    /// reach". That is the shape the engine can enforce without a
    /// grammar for triggers, and it is the shape the action is nearly
    /// always used in: the archer watching a doorway.
    Readied,
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
    /// Already carrying Aid's +5 to hit-point maximum and current hit
    /// points. A marker and nothing else — no consumer reads it for a
    /// modifier, because Aid's effect is a change to the sheet
    /// (`bump_max_hp`) rather than something recomputed from the
    /// condition each time it is asked for.
    ///
    /// It exists to enforce PHB's stacking rule: "the effects of the
    /// same spell cast multiple times don't combine." Without it, Aid
    /// added five hit points every time it was cast, on the spell and on
    /// the scroll alike — and an AI support caster, which re-runs its
    /// ladder every turn and sees the same wounded ally, spent its whole
    /// slot table re-buffing one target five points at a time. That is
    /// both against the rule and the worst available use of the turns.
    ///
    /// Deliberately *not* on `is_dispellable_buff`. Every other entry on
    /// that list is read live — an AC bonus, a save bonus, a damage
    /// clamp — so removing the condition removes the effect. Removing
    /// this one would leave the five hit points exactly where they are
    /// and merely re-open the door to another five, which is the
    /// opposite of what dispelling should do.
    ///
    /// Timer is the same long `Rounds` count Mage Armor uses for the
    /// same reason: RAW's duration is 8 hours, which no encounter
    /// reaches, and a marker that lapses mid-fight would re-open the
    /// stack it exists to close.
    Aided,
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
    /// a `Dueled` back-link so the engine knows who the duel is anchored on.
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
    /// Exhausted — the flag half of 5e's six-rung exhaustion ladder.
    /// Present exactly when `ActorInstance::exhaustion_level()` is
    /// non-zero; the tier itself is the number, and the two are held in
    /// step by `add_condition` / `remove_condition`.
    ///
    /// Which is why this variant carries almost no mechanics of its own.
    /// It sits on none of the roll-mode cohorts, because every penalty
    /// RAW attaches to exhaustion is attached to a *rung*, and the flag
    /// is up from the first one: tier 1 is disadvantage on ability
    /// checks, tier 2 halves speed, tier 3 reaches attack rolls and
    /// saving throws, tier 4 halves the hit point maximum, tier 5 is
    /// speed 0, tier 6 is death. Each gate reads the number.
    ///
    /// The flag still earns its place: it is what makes exhaustion
    /// visible to everything that speaks in conditions — immunity
    /// (celestials, constructs, undead), the cleanse pickers, the status
    /// panel — without any of them having to learn about tiers. Adding a
    /// level to a creature immune to the condition is impossible for the
    /// ordinary reason: `add_condition` bounces on immunity before it
    /// reaches the ladder.
    ///
    /// Unlike every other condition here, removing it walks one rung
    /// down rather than clearing outright — which is what both of RAW's
    /// cleanses (Greater Restoration, a long rest) actually say.
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
    /// Spider Climb (5e level-2 transmutation, concentration). The target
    /// gains a climbing speed equal to their walking speed — they can move
    /// up walls and ceilings without needing climbing checks. We don't
    /// model 3D terrain, so we surface the climb speed as a flat +30ft
    /// (12 tiles) speed bump via `speed()` — smaller than Fly's +60ft but
    /// still meaningful kiting / repositioning fuel. Concentration-bound on
    /// the caster; the spell drops the buff when concentration ends. Joins
    /// `is_dispellable_buff` so Dispel Magic / Counterspell can rip it.
    SpiderClimbing,
    /// Dominated (5e Dominate Person, level-5 enchantment, concentration).
    /// The target's will is overridden by the caster. We model the
    /// load-bearing half: the target is Charmed by the caster (so they
    /// can't attack them, via the existing `Charmed` back-link) AND any
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
    /// Invoked Duplicity (5e **Trickery Domain Cleric** Channel
    /// Divinity, subclass level 2). A perfect illusory double of the
    /// cleric stands on the battlefield; RAW hands the cleric advantage
    /// on attack rolls against any creature within 5 ft of it, so long
    /// as both the cleric and the illusion are within 5 ft of that
    /// creature.
    ///
    /// The engine models the double as a flag on the cleric rather than
    /// as a second body: there is no actor to place, move (RAW's bonus
    /// action) or knock down, and the positional clause collapses to
    /// "the cleric has advantage on attack rolls" for the duration.
    /// That is a real simplification and it is the generous direction —
    /// RAW's version can be defeated by spreading out, this one cannot.
    /// It is priced accordingly: one charge per short rest, and the
    /// Trickery cleric's whole Channel Divinity budget.
    ///
    /// Joins `grants_self_attack_advantage` as a *persistent* member
    /// (like `Transformed` and `Foreseen`, unlike the one-shot
    /// `TidesOfChaos` / `Helped` / `Hidden` primes) — it is deliberately
    /// off `CONSUMED_ON_ATTACK` so the advantage rides every swing for
    /// the ten rounds it lasts, which is the entire feature.
    ///
    /// Not on `is_dispellable_buff`: Channel Divinity is a class
    /// feature, not a spell, so Dispel Magic has nothing to grab —
    /// the same call `WildShaped` makes.
    Duplicity,
    /// Has already spent a spell slot calling for reinforcements this
    /// encounter.
    ///
    /// Pure bookkeeping, installed on the *caster* by
    /// `spawn_adjacent_summons` the first time a summon **cast from a
    /// slot** actually puts a body on the board, and read by exactly one
    /// consumer: the AI's summon rung, which won't spend a second slot
    /// while it is up.
    ///
    /// The slot leg matters. A per-rest *feature* summon — the Ranger's
    /// Companion, the Wildfire druid's spirit, the Fathomless warlock's
    /// tentacle — leaves no mark, because each already carries its own
    /// once-per-rest cap and RAW is explicit that a feature summon and a
    /// conjured pack can share a board. The marker used to go up for
    /// those too, which meant a Beast Master who whistled up their wolf
    /// on round one was silently barred from Conjure Animals for the
    /// rest of the fight.
    ///
    /// It exists because the summons disagree about what stops them
    /// from being cast again. Conjure Animals, Conjure Elemental and
    /// Animate Objects hold concentration, so one is the natural cap;
    /// the Ranger's Companion has a per-rest charge. **Animate Dead has
    /// neither** — RAW it is a permanent minion with no concentration,
    /// which is correct for the spell and catastrophic for a rung that
    /// only knows how to ask "am I concentrating?". A wizard would
    /// spend every third-level slot it owned on skeletons and never
    /// cast anything else.
    ///
    /// Deliberately on the caster rather than counted off the minions:
    /// a summoned ally that dies (or a concentration that drops) leaves
    /// the marker in place, so the caster doesn't respond to losing its
    /// wolves by immediately conjuring more while the fight is going
    /// badly. That is the conservative direction, and it is the right
    /// one for a resource this expensive.
    ///
    /// Not a restriction on the player: it gates one AI picker, and the
    /// action itself validates and executes exactly as before. A human
    /// who wants a second skeleton can still ask for one.
    ///
    /// `Rounds(100)` — longer than any encounter the engine runs, which
    /// is the honest way to spell "for this fight" in a timer model
    /// that has no such unit.
    Summoner,
    /// Divine Strike primed (5e Cleric Channel Divinity flavor; we model
    /// the level-8-and-above feature as a once-per-rest prime that lands
    /// on the caster's next melee hit for +1d8 radiant damage. Mirrors
    /// the Smiting / SearingSmiting one-shot prime pattern. Cleared by
    /// the OnHitRider table the moment the rider lands. Tick-down timer
    /// keeps a swing-less prime from dangling indefinitely.
    DivineStriking,
    /// Divine Strike (poison) primed (5e **Trickery Domain Cleric**,
    /// subclass level 8). The poison-typed arm of the same feature
    /// `DivineStriking` carries in radiant: a once-per-rest bonus-action
    /// prime that lands +1d8 poison on the cleric's next melee hit.
    ///
    /// A distinct condition rather than a damage-type field on the
    /// existing one because the `ON_HIT_RIDERS` table is keyed by
    /// condition — that is where every per-hit rider in the engine
    /// declares its dice, typing and melee gate, and a second typing
    /// is a second row there. The combat log also stays honest: a
    /// Trickery cleric's swing reads "envenomed", not "radiant fury".
    ///
    /// On `is_smite_prime` alongside `DivineStriking`, so the
    /// dispellable-buff sweep and the AI's don't-double-prime gates
    /// treat the two identically.
    DivineStrikingPoison,
    /// Fangs of the Fire Snake primed (5e **Way of the Four Elements
    /// Monk**, elemental discipline). The monk's arms are wreathed in
    /// flame: the next melee hit lands +1d10 fire.
    ///
    /// RAW also extends the unarmed strike's reach by 10 ft for the
    /// turn. That half is not modeled — reach is declared per-action
    /// by `Action::reach_tiles` and the engine's only per-swing reach
    /// override is wired specifically to the Battle Master's
    /// `LungingAttacking`. The damage half is the one that pays for the
    /// ki point on a 1d8 chassis.
    ///
    /// Not on `is_smite_prime`: that cohort is the paladin / cleric
    /// divine-smite family, and a monk's elemental discipline is
    /// neither divine nor dispellable (it is a class feature, not a
    /// spell). It rides `ON_HIT_RIDERS` and the two-round tick-down
    /// like every other prime, and nothing else.
    FangsOfTheFireSnake,
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
    /// Invested with Ice (5e level-6 transmutation, concentration). The
    /// caster's body is sheathed in shards of ice: they gain resistance to
    /// cold damage and any creature within reach that hits them with a
    /// melee attack takes 1d10 cold damage in retaliation. Symmetric to
    /// `InvestedInFlame` (same shape, swapped element); the install lives
    /// in `INVESTITURE_OF_ICE` and the retaliation rider lives in
    /// `MELEE_REFLECT_RIDERS` next to the InvestedInFlame entry. Cleared
    /// on concentration drop.
    InvestedInIce,
    /// Invested with Stone (5e level-6 transmutation, concentration). The
    /// caster's body hardens to living rock: they gain resistance to
    /// bludgeoning, piercing, and slashing damage (the three physical
    /// weapon types) and any creature within reach that hits them with a
    /// melee attack takes 1d10 force damage in retaliation. Sibling to
    /// `InvestedInFlame` / `InvestedInIce` — same install + reflect
    /// shape, with a broader resistance envelope (all physical) instead
    /// of a single damage type. Concentration-bound on the caster;
    /// dropping concentration drops the buff.
    InvestedInStone,
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
    /// concentration), by one of the two growth potions, or by any future
    /// growth effect. The target's size category bumps up by one — really,
    /// on the board: the condition is a row on `RESIZING_CONDITIONS`, so
    /// `desired_size` asks for the larger footprint and the engine's
    /// reconciler stamps it once there is room. They also roll +1d4 extra
    /// damage on weapon attacks (read by the on-hit rider table) and save
    /// with advantage on STR.
    ///
    /// Concentration-bound on the caster when it comes from the spell;
    /// dropping concentration drops the buff, and the actor shrinks back
    /// on the next pump.
    Enlarged,
    /// Reduced by the Reduce half of Enlarge / Reduce (5e level-2
    /// transmutation, concentration) — the symmetric debuff to `Enlarged`
    /// and its mirror image on every lane: one size category down, -1d4
    /// on weapon damage, disadvantage on STR saves.
    ///
    /// The two cancel rather than fight: `desired_size` sums the ladder
    /// steps, so a creature holding both sits at its base size, which is
    /// RAW's "no effect on a creature already under the other's
    /// influence" reached by arithmetic instead of by a precedence rule.
    Reduced,
    /// **Giant's Might** (5e Rune Knight Fighter, subclass level 3).
    /// A bonus action spent drawing on giant blood: for a minute the
    /// fighter grows one size category, saves with advantage on STR, and
    /// once on each of their turns a connecting weapon hit carries an
    /// extra 1d6.
    ///
    /// The first *class feature* to ride `RESIZING_CONDITIONS` — every
    /// other size effect in the engine is a spell or a potion — and the
    /// reason the size lane earns its keep on a martial chassis: growing
    /// widens the fighter's own footprint, which widens the envelope in
    /// which they threaten opportunity attacks.
    ///
    /// On `is_dispellable_buff`: it is explicitly magical in RAW ("you
    /// magically become larger"), unlike the maneuver primes it sits
    /// beside on the fighter's sheet.
    GiantsMight,
    /// **Fire Rune** invoked (5e Rune Knight Fighter, subclass level 3).
    /// A bonus action spent kindling the rune etched on the fighter's
    /// weapon: the next hit burns for an extra 2d6 fire and forces a STR
    /// save, and a failure wraps the target in chains of fire.
    ///
    /// A prime, not a buff — it describes what the fighter's next swing
    /// will do and is consumed by that swing. Off `is_dispellable_buff`
    /// for the same reason the maneuver primes are.
    FireRuneInvoked,
    /// **Reaper's Touch** primed (5e Death Domain Cleric, subclass level
    /// 2, RAW "Touch of Death"). The cleric's Channel Divinity spent on
    /// a bonus action: their next melee hit carries a slab of extra
    /// necrotic damage.
    ///
    /// Sibling to `DivineStriking` and `DivineStrikingPoison` on the
    /// cleric's melee-prime lane, three times the die.
    TouchingDeath,
    /// **Divine Strike (necrotic)** primed (5e Death Domain Cleric,
    /// subclass level 8) — the domain's typing of the shared cleric
    /// feature. Identical in every respect to `DivineStriking` except
    /// the damage type its rider row carries.
    DivineStrikingNecrotic,
    /// **Divine Strike (psychic)** primed (5e Order Domain Cleric,
    /// subclass level 8) — the fourth typing of the same feature, after
    /// radiant, poison and necrotic. Same rider row, different type.
    DivineStrikingPsychic,
    /// Bonded by Warding Bond (5e level-2 abjuration). The bonded actor
    /// gains +1 AC, +1 saving throws, and resistance to all damage. Any
    /// damage that lands on the bonded actor is mirrored onto their
    /// bonding partner (the caster) at the post-resistance amount; the
    /// `WardingBonded` back-link on `ActorInstance` carries the partner id so
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
    /// Ensnaring Strike primed (5e level-1 ranger conjuration,
    /// concentration). The ranger's next weapon attack hit — either
    /// melee or ranged — deals +1d6 piercing via the on-hit rider table,
    /// and thorny vines sprout at the point of impact: the target makes
    /// a STR save vs the ranger's WIS-based DC or is Restrained until
    /// the spell ends. One-shot prime — the rider table strips this
    /// flag the moment the consuming hit lands. Ranger's lv1 sibling to
    /// the paladin's Wrathful Smite prime (also lv1, also STR-save-vs-
    /// condition follow-up, but Wrathful is CHA-anchored + melee-only +
    /// psychic-typed + Frightened; Ensnaring is WIS-anchored + both
    /// lanes + piercing-typed + Restrained). Unlike Lightning Arrow
    /// (ranged-only), Ensnaring Strike fires on either the scimitar
    /// swing or the longbow shot — matching RAW's "the next time you
    /// hit a creature with a weapon attack" broad envelope.
    EnsnaringStriking,
    /// Zephyr Strike primed (5e level-1 ranger transmutation, XGtE,
    /// concentration). The ranger's next weapon attack hit — either
    /// melee or ranged (RAW's "the next attack you make on this turn"
    /// broad envelope, unrestricted by weapon lane) — deals +1d8 force
    /// damage via the on-hit rider table. One-shot prime — the rider
    /// table strips this flag the moment the consuming hit lands.
    /// Ranger's lv1 pure-damage sibling to Ensnaring Strike (lv1
    /// piercing + Restrained follow-up) and Lightning Arrow (lv3
    /// lightning, ranged-only): same SmiteSpell chassis, no save
    /// follow-up, no ranged gate, and Force-typed (one of the rarest-
    /// resisted damage types in the engine — the rider punches through
    /// nearly every typed-defense lane cleanly). RAW's companion
    /// clauses (advantage on the primed attack, +30 ft speed for the
    /// turn, and no opportunity attacks provoked while the spell is
    /// up) are left as future work — a per-hit advantage rider needs a
    /// caster-side attack-mode chokepoint the engine doesn't yet
    /// expose, and the movement + OA-immunity clauses fold into the
    /// same turn-scoped kite envelope the ranger's baseline Fighting
    /// Style: Archery + Vanish already lean on. The +1d8 force damage
    /// is the load-bearing tactical clause and rides here alone.
    ZephyrStriking,
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
    /// Menacing Attack primed (5e Fighter Battle Master maneuver, once
    /// per long rest in our model). Bonus action prime; the next melee
    /// weapon hit forces the target to make a WIS save vs the fighter's
    /// maneuver DC (8 + prof + STR); on fail, they're Frightened until
    /// the end of the fighter's next turn. One-shot — the OnHitRider
    /// table strips this flag the moment a melee swing lands. Tick-down
    /// timer (2 rounds) caps the prime if the fighter can't connect.
    /// Mirrors Trip Attack's shape but with a WIS save and Frightened
    /// instead of Prone.
    MenacingAttacking,
    /// Disarming Attack primed (5e Fighter Battle Master maneuver, once
    /// per long rest in our model). Bonus action prime; the next melee
    /// weapon hit forces the target to make a STR save vs the fighter's
    /// maneuver DC; on fail, they're Disarmed (attacks have disadvantage
    /// until the start of their next turn). One-shot — the rider table
    /// strips this flag the moment a melee swing lands.
    DisarmingAttacking,
    /// Pushing Attack primed (5e Fighter Battle Master maneuver, once
    /// per long rest in our model). Bonus action prime; the next melee
    /// weapon hit forces the target to make a STR save vs the fighter's
    /// maneuver DC; on fail, they're shoved 10 ft (4 tiles) away via
    /// the standard `PushActor` helper. One-shot — the rider table
    /// strips this flag the moment a melee swing lands.
    PushingAttacking,
    /// Disarmed (5e Battle Master Disarming Attack rider). The target's
    /// weapon was knocked from their grip: their attack rolls have
    /// disadvantage (joins `imposes_attacker_disadvantage`). Short
    /// timer (`UntilStartOfNextTurn`) clears the debuff after the
    /// holder's next turn — RAW: lasts until the target picks the
    /// weapon back up, which any creature can do as part of a move.
    Disarmed,
    /// Goading Attack primed (5e Fighter Battle Master maneuver, once
    /// per long rest in our model). Bonus action prime; the next melee
    /// weapon hit forces the target to make a WIS save vs the fighter's
    /// STR-based maneuver DC; on fail, they're Goaded — attacks against
    /// anyone other than the fighter are at disadvantage. One-shot —
    /// the OnHitRider table strips this flag the moment a melee swing
    /// lands.
    GoadingAttacking,
    /// Goaded (5e Battle Master Goading Attack rider). The target was
    /// goaded into focusing on the fighter who hit them: attack rolls
    /// against anyone *other* than the goading fighter are at disadvantage.
    /// Engine reads the `Goaded` back-link on the holder to identify the
    /// fighter (mirrors how `Dueled` + `Dueled` back-link route through
    /// `compute_attack_mode`). Short timer
    /// (`UntilStartOfNextTurn`) — RAW: lasts until the end of the
    /// fighter's next turn.
    Goaded,
    /// Precision Attack primed (5e Fighter Battle Master maneuver, once
    /// per long rest in our model). Bonus action prime; the next attack
    /// roll gains a flat +4 (modeling the +1d8 superiority die, d8 avg
    /// rounded down). Consumed by `clear_attack_advantage_riders` the
    /// moment the attack roll resolves, mirroring how Bardic Inspiration
    /// (`Inspired`) is consumed. RAW lets you spend the die *after*
    /// seeing the d20 result; we approximate the "did it land?" tactic
    /// by applying the bonus before the roll so the AI can use it as a
    /// proactive accuracy buff. One-shot — the next attack consumes it.
    PrecisionAttacking,
    /// Sweeping Attack primed (5e Fighter Battle Master maneuver, once
    /// per long rest in our model). Bonus action prime; the next melee
    /// weapon hit splashes a small amount of slashing damage onto one
    /// adjacent enemy of the primary target (RAW: superiority-die damage
    /// of the same type as the original attack; we collapse to 1d8
    /// slashing since the engine's rider table doesn't carry per-weapon
    /// typing into the splash). The splash bypasses the usual "must hit"
    /// gate — RAW: it lands as long as the original attack hit. One-shot
    /// — the rider table strips this flag the moment a melee swing
    /// lands.
    SweepingAttacking,
    /// Lunging Attack primed (5e Fighter Battle Master maneuver, once per
    /// long rest in our model). The fighter's next melee weapon attack
    /// gains +5 ft of reach (one extra tile in our 2.5ft grid, doubling
    /// melee reach from 1 to 2). Engine reads via
    /// `ActorInstance::extra_melee_reach()` which the action_template's
    /// reach check folds into the effective range. One-shot — consumed
    /// by `clear_attack_advantage_riders` on the next melee swing.
    /// Tick-down timer (`UntilStartOfNextTurn`) caps an unused prime so
    /// it doesn't sit across rounds.
    LungingAttacking,
    /// Empowered Spell primed (5e Sorcerer Metamagic). The sorcerer has
    /// spent a sorcery point via the Empowered Spell bonus-action prime;
    /// the next spell damage roll they make can reroll up to CHA-mod
    /// dice whose face came up at 1 or 2. We approximate the "reroll?"
    /// choice with the obvious tactical answer: always reroll dice <= 2
    /// since the expected reroll average is `(faces+1)/2` (>= 2.5 for
    /// any d4+ die — the spell-damage dice we care about).
    /// One-shot — consumed the moment a spell damage roll lands via
    /// `EncounterInstance::roll_empowered`. Tick-down timer
    /// (`UntilStartOfNextTurn`) caps an unused prime so it doesn't dangle.
    EmpoweredSpelling,
    /// Heightened Spell primed (5e Sorcerer Metamagic). The sorcerer has
    /// spent three sorcery points via the Heightened Spell bonus-action
    /// prime; the first creature that makes a saving throw against the
    /// sorcerer's next spell rolls that save at disadvantage. RAW
    /// applies only to the first save (single target or first burst
    /// victim in initiative order) — we honor that by consuming the
    /// prime on the first save resolved through
    /// `EncounterInstance::roll_save_against_caster`.
    /// Tick-down timer (`UntilStartOfNextTurn`) caps an unused prime so
    /// it doesn't dangle across rounds.
    HeightenedSpelling,
    /// Careful Spell primed (5e Sorcerer Metamagic). The sorcerer has
    /// spent one sorcery point via the Careful Spell bonus-action prime;
    /// the next AoE the sorcerer casts spares allies caught in the
    /// blast — up to CHA-mod of them auto-pass their save AND take no
    /// damage (RAW: "A chosen creature automatically succeeds on its
    /// saving throw against the spell, and it takes no damage if it
    /// would normally take half damage on a successful save"). Read at
    /// the burst-resolver chokepoints (`burst_save_damage` in spells.rs
    /// and `resolve_burst_save_damage` in action_template.rs); consumed
    /// the first time a burst lands. Tick-down timer
    /// (`UntilStartOfNextTurn`) caps an unused prime.
    CarefulSpelling,
    /// Distant Spell primed (5e Sorcerer Metamagic). The sorcerer has
    /// spent one sorcery point via the Distant Spell bonus-action prime;
    /// the next ranged spell they cast has its range doubled (or, for a
    /// touch-range spell, jumps to 30 ft / 12 tiles per RAW; we collapse
    /// to "doubled reach with a floor of 12 if the base reach is 1").
    /// Engine reads via `ActorInstance::extra_spell_reach()` which the
    /// action_template's reach check folds into the effective range.
    /// Consumed in `Action::execute` after a ranged action validates and
    /// fires (gated to reach > 1 so a melee swing can't burn the prime).
    DistantSpelling,
    /// Twinned Spell primed (5e Sorcerer Metamagic). The sorcerer has set
    /// up the Twinned Spell bonus-action prime; the next single-target
    /// spell they cast will fire a second time against a different valid
    /// target in the same range. RAW: SP cost equals the spell's level
    /// (cantrip = 1 SP), and the spell must target only one creature
    /// (range != self). Engine reads via
    /// `EncounterInstance::consume_twinned_spell` in `Action::execute`,
    /// which scans the action's cost for a `SpellSlot(lvl)` to size the
    /// SP debit, picks a sensible second target (nearest opposing-team
    /// creature for harmful spells, lowest-HP ally for buffs / heals),
    /// and re-fires the action's `side_effects` against it. The prime is
    /// consumed only when a second target is actually engaged so a
    /// twinned cast on a solo enemy isn't wasted. Tick-down timer
    /// (`UntilStartOfNextTurn`) caps an unused prime.
    TwinnedSpelling,
    /// Extended Spell primed (5e Sorcerer Metamagic). The sorcerer has
    /// spent one sorcery point via the Extended Spell bonus-action prime;
    /// the next spell they cast that installs a long-duration condition
    /// (RAW: 1 minute or longer; we gate on `Rounds(n)` with n >= 10)
    /// has its timer doubled. Read at the side-effect-assembly chokepoint
    /// in `Action::execute`: any `ApplyCondition` whose `extend_duration`
    /// returns true (i.e. an eligible long timer was doubled) consumes
    /// the prime. Spells whose only effects are short-duration buffs or
    /// instantaneous damage don't burn the prime — it dangles until the
    /// next eligible cast or the tick-down expires. Tick-down timer
    /// (`UntilStartOfNextTurn`) caps an unused prime so it doesn't sit
    /// across rounds.
    ExtendedSpelling,
    /// Seeking Spell primed (5e Tasha's Sorcerer Metamagic). The sorcerer
    /// has spent two sorcery points via the Seeking Spell bonus-action
    /// prime; the next spell-attack roll the sorcerer makes that misses is
    /// rerolled (RAW: "you can spend 2 sorcery points to reroll the d20").
    /// Read at the spell-attack chokepoint (`spell_attack_outcome` in
    /// spells.rs) which calls `EncounterInstance::reroll_seeking_spell` on
    /// a miss; the prime is consumed the moment the reroll lands (whether
    /// the new roll hits or misses — RAW: "you must use the new roll").
    /// Tick-down timer (`UntilStartOfNextTurn`) caps an unused prime so
    /// it doesn't dangle across rounds.
    SeekingSpelling,
    /// Subtle Spell primed (5e Sorcerer Metamagic). The sorcerer has spent
    /// one sorcery point via the Subtle Spell bonus-action prime; their
    /// next spell ignores Counterspell (RAW: "you can cast it without any
    /// somatic or verbal components" — Counterspell needs to perceive the
    /// cast, so a no-component cast slips past). The engine reads this at
    /// `Counterspell::custom_validate_input` (spells.rs): if the targeted
    /// caster has the prime up, the validate returns false and the
    /// counterspell fizzles. The prime is consumed inside the same
    /// validator on the failed cast (mirrors `clear_attack_advantage_riders`'s
    /// consume-on-trigger pattern). Tick-down timer (`UntilStartOfNextTurn`)
    /// caps an unused prime so it doesn't dangle across rounds.
    SubtleSpelling,
    /// Tides of Chaos primed (5e Wild Magic Sorcerer feature). The
    /// sorcerer has spent their once-per-long-rest charge to gain
    /// advantage on their next attack roll, ability check, or saving
    /// throw. We honor the attack-roll lane: holders count as having
    /// `grants_self_attack_advantage` (RollMode::Advantage on the next
    /// swing) and the condition lives in `CONSUMED_ON_ATTACK` so it
    /// burns off the first swing that fires. Tick-down timer
    /// (`UntilStartOfNextTurn`) caps an unused prime so it doesn't
    /// dangle across rounds.
    TidesOfChaos,
    /// Transmuted Spell primed (5e Tasha's Sorcerer Metamagic). The sorcerer
    /// has spent one sorcery point via the Transmuted Spell bonus-action
    /// prime; the next spell whose damage type is one of the "elemental six"
    /// (acid, cold, fire, lightning, poison, thunder) is remapped to a
    /// different element from that same list (RAW: "When you cast a spell
    /// that deals a type of damage from the following list, you can spend 1
    /// sorcery point to change that damage type"). The engine reads the
    /// prime at the side-effect-assembly chokepoint in `Action::execute`:
    /// `consume_transmuted_spell` walks the spell's `DealDamage` entries,
    /// picks the target's worst weakness (vulnerability > non-resisted >
    /// best-non-immune) among the six elemental types, and remaps the
    /// damage type in place. Consumed on the first remap; spells whose only
    /// damage is non-elemental (force / radiant / necrotic / psychic /
    /// physical) leave the prime up for the next eligible cast. Tick-down
    /// timer (`UntilStartOfNextTurn`) caps an unused prime so it doesn't
    /// sit across rounds.
    TransmutedSpelling,
    /// Cunning Strike: Poison primed (5e 2024 Rogue lv5 feature). The
    /// rogue has spent a bonus action committing to the poison variant of
    /// Cunning Strike; their next Sneak Attack rider trades one d6 of
    /// damage for an attempt to poison the target. The shortsword reads
    /// the prime, deducts 1d6 of sneak dice, and the target makes a CON
    /// save vs the rogue's DEX-based DC; on fail they're `Poisoned` for
    /// `Rounds(10)` (1 minute RAW). Consumed the moment a sneak attack
    /// fires (or, if the sneak doesn't trigger this turn, the
    /// `UntilStartOfNextTurn` timer drops the prime so it doesn't dangle).
    CunningStrikePoison,
    /// Cunning Strike: Trip primed (5e 2024 Rogue lv5 feature). Bonus-
    /// action prime; the next sneak-attack swing trades one d6 of damage
    /// for a DEX save (target Large or smaller). On fail the target is
    /// knocked Prone. Consumed on sneak-attack trigger or
    /// `UntilStartOfNextTurn`.
    CunningStrikeTrip,
    /// Cunning Strike: Withdraw primed (5e 2024 Rogue lv5 feature).
    /// Bonus-action prime; the next sneak-attack swing trades one d6 of
    /// damage to immediately move up to half the rogue's speed without
    /// provoking opportunity attacks (Disengage envelope). Consumed
    /// on sneak-attack trigger or `UntilStartOfNextTurn`.
    CunningStrikeWithdraw,
    /// Cunning Strike: Daze primed (5e 2024 Rogue lv5 feature). Bonus-
    /// action prime; the next sneak-attack swing trades two d6 of damage
    /// (RAW: 2d6 cost) for a CON save. On fail the target's next turn
    /// loses its Action and reaction (we model via `MindWhipped`'s
    /// action-economy clip plus `NoReaction`). Consumed on sneak-attack
    /// trigger or `UntilStartOfNextTurn`.
    CunningStrikeDaze,
    /// Caustic-Brewed (5e Tasha's Caustic Brew, level-1 evocation). The
    /// target is splashed with magical acid that clings to skin / scales:
    /// they take 2d4 acid at the end of each of their turns until they (or
    /// an ally adjacent to them) use an Action to scrape it off, OR the
    /// spell's duration ends. We model the DoT via the standard
    /// `ROUND_END_DOTS` registry (2d4 acid per round) and the
    /// scrape-off via the cleanse-style `WipeAcid` action — mirrors
    /// `StillnessOfMind`'s self-clean envelope. Concentration-bound on
    /// the caster RAW; we install with a `Rounds(10)` timer (~1 minute
    /// RAW) so dropping concentration severs the drip cleanly. Distinct
    /// from `VitriolicAcidCoated` (a one-shot drip from Vitriolic Sphere)
    /// so the cleanse action and concentration cleanup target just this
    /// mark.
    CausticBrewed,
    /// Distracting Strike primed (5e Fighter Battle Master maneuver, once
    /// per long rest in our model). Bonus-action prime that adds +1d6
    /// damage to the next melee weapon hit and tags the target as
    /// `Distracted` — the next attack against that target by an attacker
    /// other than the fighter has advantage. One-shot — the rider table
    /// strips this flag the moment a melee swing lands. Tick-down timer
    /// (`UntilStartOfNextTurn`) caps a swing-less prime so it doesn't
    /// sit across rounds. Mirrors Goading Attack's prime+target-debuff
    /// shape but with a target-side advantage rider instead of
    /// attacker-side disadvantage.
    DistractingAttacking,
    /// Distracted (5e Battle Master Distracting Strike rider). The
    /// target's guard has been compromised: attack rolls against them by
    /// any attacker *other* than the fighter who tagged them have
    /// advantage. Engine reads the `Distracted` back-link on the holder to
    /// identify the fighter (mirrors how `Goaded` + `Goaded` back-link route
    /// through `compute_attack_mode`, but as a target-side rider rather
    /// than an attacker-side one). Short timer
    /// (`UntilStartOfNextTurn`) — RAW: lasts until the start of the
    /// fighter's next turn; we use the target-side tick-down envelope
    /// shared with Mocked / Helped / Goaded.
    Distracted,
    /// Eldritch Struck (5e Eldritch Knight Fighter **Eldritch Strike**,
    /// subclass level 10). The knight's blade has rattled the target's
    /// footing for the spell that follows it: the next saving throw the
    /// target makes against a spell cast *by the knight who hit them* is
    /// rolled at disadvantage. Engine reads the `EldritchStruck` back-link
    /// link on the holder at the shared `CASTER_SAVE_MODE_RIDERS` cohort
    /// in `roll_save_against_caster` — the same flag-plus-link shape as
    /// `Distracted` + `Distracted` back-link and `Sworn` + `Sworn` back-link, but on
    /// the save-roll axis rather than the attack-roll one, and
    /// positive-polarity on the link like `Sworn` (a *match* is what
    /// fires the rider; a different caster's spell reads clean).
    ///
    /// One-shot: the cohort's `consume` strips the mark the moment it
    /// bends a save, so a knight who hits twice and then casts a
    /// multi-target spell still only debuffs the first save. RAW's
    /// window is "before the end of your next turn"; we install with
    /// `Rounds(2)` so an unspent mark decays on its own rather than
    /// dangling.
    ///
    /// Deliberately *not* on the `is_dispellable_buff` sweep — the mark
    /// is a mundane weapon rattle in RAW's fiction (the Eldritch Knight
    /// hits you with a sword; the magic is in what comes next), and the
    /// engine's dispel lane only reaches spell effects.
    EldritchStruck,
    /// War Magic primed (5e Eldritch Knight Fighter **War Magic**,
    /// subclass level 7). The knight has spent their Action on a cantrip
    /// this turn, which unlocks the `WAR_MAGIC_STRIKE` bonus action for
    /// a follow-up swing. Installed by the post-cast hook
    /// `trigger_war_magic_prime` on any `spell_level == 0` cast by a
    /// `WAR_MAGIC_TAG` holder; consumed by the bonus action itself.
    ///
    /// Tick-down timer (`UntilStartOfNextTurn`) — this is what enforces
    /// RAW's same-turn window rather than a bespoke ledger: the timer
    /// clears at the start of the holder's next turn, so an uncashed
    /// prime can never fund a swing on a later turn's bonus action.
    /// Same one-shot-prime envelope as the metamagic primes
    /// (`HeightenedSpelling`, `SubtleSpelling`, ...) and the Cunning
    /// Strike primes, differing only in what installs it: a completed
    /// cast rather than a resource spend.
    WarMagicPrimed,
    /// Shadow Step primed (5e Way of Shadow Monk **Shadow Step**,
    /// subclass level 6). The monk has just stepped through shadow, and
    /// RAW hands them "advantage on the first melee attack you make
    /// before the end of the turn."
    ///
    /// The engine's only melee-gated self-advantage rider — it rides
    /// `grants_self_melee_attack_advantage` rather than the broader
    /// `grants_self_attack_advantage` its one-shot siblings
    /// (`TidesOfChaos`, `Helped`, `Hidden`) use, because the RAW clause
    /// is explicit about melee and the monk has thrown-weapon options.
    ///
    /// Consumed by `CONSUMED_ON_ATTACK` on the first swing that fires.
    /// That cohort doesn't distinguish lanes, so a ranged swing burns
    /// the prime without collecting it — the same deviation
    /// `LungingAttacking` already documents and accepts, and a near-
    /// moot one on a chassis whose entire kit is unarmed strikes.
    /// Tick-down timer (`UntilStartOfNextTurn`) caps an unspent prime so
    /// the advantage can't carry into the next round.
    Shadowstepping,
    /// Wild Shaped (5e Druid **Wild Shape**, as accelerated by Circle of
    /// the Moon's **Combat Wild Shape**). The druid is wearing a beast's
    /// body: they carry the form's hit points as a temp HP pool, swing
    /// the form's natural weapons via `BEAST_FORM_CLAWS`, and — RAW's
    /// load-bearing cost — **can't cast spells**.
    ///
    /// That last clause rides `blocks_spell_slots`, the same gate the
    /// Silence spell's `Silenced` uses. The two arrive at the identical
    /// mechanical place from opposite directions (a hush imposed on you
    /// versus a body you chose), which is exactly why the gate is a
    /// cohort rather than a `Silenced` special-case: a full-caster who
    /// spends a bonus action to become a bear is trading their entire
    /// spell list for a melee chassis, and the engine should say so with
    /// the same machinery either way.
    ///
    /// The one thing that clause deliberately does *not* block is
    /// Combat Wild Shape's own slot-to-hit-points conversion, which RAW
    /// is careful to phrase as expending a slot rather than casting.
    /// `WildHeal` honors that by draining the slot manager directly
    /// instead of routing a `SpellSlot` cost through
    /// `can_consume_resource` — see its doc comment.
    ///
    /// `Rounds(10)` timer, the engine's standard long-buff envelope
    /// (RAW is hours). Not on `is_dispellable_buff`: Wild Shape is a
    /// class feature, not a spell, so Dispel Magic has nothing to grab.
    WildShaped,
    /// Sworn — 5e Paladin Oath of Vengeance Channel Divinity: Vow of
    /// Enmity (lv3 subclass feature, once per long rest). The target is
    /// marked as the paladin's chosen quarry: the swearing paladin (and
    /// only the swearing paladin) gets advantage on attack rolls against
    /// this target for up to 10 rounds (1 minute RAW). Engine reads the
    /// `Sworn` back-link on the holder to identify the paladin (mirrors
    /// the `Dueled` + `Dueled` back-link / `Distracted` + `Distracted` back-link flag-
    /// plus-link shape, but positive-polarity: a *match* on the link
    /// grants advantage rather than a *mismatch* imposing disadvantage).
    /// Distinct from Hunter's Mark in two ways:
    ///   1. **No damage rider** — Vow of Enmity is purely an accuracy
    ///      buff; the paladin's own smite primes carry the damage.
    ///   2. **No concentration** — the paladin can hold a smite
    ///      concentration spell (Searing / Wrathful / Branding) AND keep
    ///      the vow active at the same time.
    ///
    /// Cleared when the timer expires or the paladin is downed.
    Sworn,
    /// Purified — 5e Paladin Aura of Purity (lv4 abjuration, concentration).
    /// The holder is shielded by the paladin's protective aura: resistance
    /// to poison damage (folded into the condition_resistance lane in
    /// `effective_damage` alongside Globed / Raging), plus dynamic
    /// immunity to Charmed / Frightened / Poisoned installs (gated in
    /// `dynamic_immunity_to`, same chokepoint as Heroic / MindBlanked /
    /// Brave). Distinct from `Warded` (Protection from Evil and Good's
    /// fiend/undead-only advantage rider) — Purified is broader and
    /// allies-only, but only against the three social/biological
    /// debuffs. Applied to every ally inside the paladin's 30ft aura at
    /// cast time; concentration tracks the full list so dropping
    /// concentration strips the flag from every ally at once.
    Purified,
    /// Beset by a Phantasmal Force (5e level-2 illusion, concentration).
    /// The target perceives the illusion as real and is mentally
    /// distracted by it — they take 1d6 psychic damage at the end of
    /// each of their turns via the standard `ROUND_END_DOTS` registry
    /// (the illusion "harms" them as their mind invents wounds). The
    /// load-bearing combat clause RAW is the per-round drip; we omit
    /// the "engaged with illusion" attack-disadvantage clause since the
    /// drip is the differentiator vs the existing single-target psychic
    /// lane (Mind Sliver / Mind Spike / Phantasmal Killer). Concentration-
    /// bound on the caster; dropping concentration dispels the illusion
    /// cleanly via the standard concentration-cleanup path.
    PhantasmalForced,
    /// Trapped in a Watery Sphere (5e XGtE level-4 conjuration,
    /// concentration). The target is encased in a 5-foot sphere of
    /// water held aloft by the caster — Restrained envelope (zero
    /// movement, attack disadvantage, attacks against have advantage)
    /// AND Lifted (suspended above the ground). We collapse the two
    /// halves into a single condition that joins both `zeros_movement`
    /// and `grants_advantage_to_attackers` so the sphere's mechanical
    /// envelope reads correctly without piling two separate flags onto
    /// the holder. Concentration-bound on the caster; dropping
    /// concentration bursts the sphere and frees the target cleanly.
    /// Distinct from `Sphered` (Otiluke's Resilient Sphere, lv4
    /// evocation): Watery Sphere targets a single creature with a STR
    /// save instead of DEX, lifts them above the ground, and uses a
    /// distinct log line for the concentration cleanup.
    WaterSphered,
    /// Invested with Wind (5e XGtE level-6 transmutation, concentration).
    /// The caster is wrapped in a swirling vortex of air: ranged attacks
    /// against them have disadvantage (the wind deflects arrows / bolts
    /// just like Wind Wall — joins `imposes_disadvantage_to_ranged_attackers`),
    /// and they can hover above the ground (joins the `Flying` cohort
    /// via `is_flying`). Symmetric to Investiture of Flame / Ice / Stone
    /// — same self-only concentration-bound install shape but a different
    /// defensive envelope (ranged deflection + flight rather than damage
    /// resistance + melee retaliation). The Gust-of-Wind action RAW grants
    /// is omitted; the load-bearing combat clause is the ranged
    /// deflection plus flight.
    InvestedInWind,
    /// Caught in a Storm Sphere (5e XGtE level-4 evocation, concentration).
    /// The target was buffeted by the storm's lashing winds on cast and
    /// is still riding the residual gusts: their own ranged attacks have
    /// disadvantage (joins `imposes_attacker_disadvantage_on_ranged`)
    /// — the wind throws off bow swings / spell arrows the moment they
    /// release. Melee swings still land normally. Distinct from
    /// `WindWalled` / `InvestedInWind` (which impose disadvantage on
    /// attackers vs the holder), Storm Sphere imposes it on the holder
    /// themselves, mirroring how Frightened / Blinded penalize the
    /// holder rather than their attackers. Concentration-bound on the
    /// caster; dropping concentration ends the storm and strips the
    /// flag from every failed-save target at once.
    WindBlasted,
    /// Longstriding (5e Longstrider spell, level-1 transmutation,
    /// no concentration). The target's speed increases by 10 ft for 1
    /// hour. We route the +10 ft (= +4 tiles) through the central
    /// `condition_speed_bonus` lane so the bump composes cleanly with
    /// Fly / Spider Climb / Expeditious Retreat. Long timer (~100 rounds
    /// = 10 minutes engine time, plenty for any encounter); no
    /// concentration means the buff sits durably across multiple
    /// encounters in the same long rest. Joins `is_dispellable_buff`
    /// so Dispel Magic can rip the speed boost cleanly.
    Longstriding,
    /// Coiled (5e Fathomless Warlock **Tentacle of the Deep**): the
    /// tentacle's slam leaves its target's "speed reduced by 10 feet
    /// until the start of your next turn."
    ///
    /// The first *negative* row on the `CONDITION_SPEED_BONUSES` cohort,
    /// which is why it is a condition rather than an inline subtraction:
    /// the table sums signed feet, so a slow composes with a Longstrider
    /// or an Expeditious Retreat the same way two buffs do, and a
    /// creature that is both hastened and coiled arrives at the number
    /// RAW would give it without anybody writing the interaction down.
    ///
    /// Deliberately not `Slowed`, which is the Slow *spell* — halved
    /// speed plus -2 AC and -2 DEX saves. RAW's tentacle takes ten feet
    /// and nothing else, and reusing the heavier condition would have
    /// made a cantrip-tier bonus action into a level-3 debuff.
    ///
    /// Short timer: RAW ends it at the start of the warlock's next turn,
    /// which on this engine's round model is one round.
    Coiled,
    /// Expeditiously Retreating (5e Expeditious Retreat spell, level-1
    /// transmutation, concentration). The caster's speed jumps by 30 ft
    /// (the spell RAW lets the caster Dash as a bonus action; we collapse
    /// the action-economy half into a flat +30 ft speed bump so the kiting
    /// payoff matches a typical caster's first Dash). Routed through
    /// `condition_speed_bonus` alongside Longstriding for one chokepoint.
    /// Concentration-bound on the caster; dropping concentration ends the
    /// burst. Joins `is_dispellable_buff` so Dispel Magic / Counterspell
    /// can rip it.
    ExpeditiouslyRetreating,
    /// Flame-Arrowed (5e Flame Arrows spell, level-3 transmutation,
    /// concentration). The caster has imbued a quiver of arrows / bolts /
    /// spell-darts with elemental fire — every ranged weapon hit they land
    /// deals +1d6 fire via the on_hit_riders table. Mirrors Spirit Shroud's
    /// shape: persistent (non-consumed) per-hit rider, concentration-bound
    /// on the caster, dispel-strippable. The `ranged_only` flag in the
    /// OnHitRider table gates the rider to bow / crossbow swings so a
    /// melee fallback can't burn the buff. Cleared on concentration drop.
    FlamingArrowed,
    /// Ashardalon-Striding (5e Ashardalon's Stride, level-3 transmutation,
    /// concentration). The caster's body crackles with elemental power: their
    /// speed jumps by 20 ft (routed through `condition_speed_bonus` alongside
    /// Longstriding / Expeditious Retreat), and every footprint-adjacent
    /// enemy takes 1d6 fire damage from the blazing wake on each step. We
    /// model the trail damage on the `MoveActor` apply path next to the
    /// Spike Growth / Booming Blade riders, but symmetric — the *caster*
    /// damages adjacent enemies, not the mover. Concentration-bound on the
    /// caster; dropping concentration drops the buff. Joins `is_dispellable_buff`.
    AshardalonStriding,
    /// Silenced (5e Silence spell, level-2 illusion, no concentration). A
    /// 20ft sphere of magical silence covers the holder: no sound is made
    /// inside, so the holder cannot cast any spell that has a verbal
    /// component. We approximate the "no V-component spells" RAW clause by
    /// blocking SpellSlot resource consumption — all leveled spells in
    /// 5e have V components by default, and the few S-only spells in the
    /// SRD are utility / out-of-combat (Find Familiar, Identify) so the
    /// collapse is faithful for combat. Cantrips are unaffected (no slot
    /// cost). Holders are also immune to thunder damage (the magical
    /// silence absorbs sonic effects) — folded into `effective_damage` via
    /// the condition lane. Tracked as a condition with a flat `Rounds(10)`
    /// timer (1 minute RAW); the spell installs on every actor caught in
    /// the burst at cast time. Distinct from `Deafened` (no spellcasting
    /// gate; the caster just can't hear).
    Silenced,
    /// Free of bodily restraint (5e Freedom of Movement, level-4
    /// abjuration). The target ignores difficult terrain and is dynamically
    /// immune to Paralyzed / Restrained / Grappled installs (gated in
    /// `dynamic_immunity_to`, same chokepoint as Heroic / MindBlanked).
    /// The spell also frees the target from any active install of those
    /// three conditions at cast time — the install side-effect issues
    /// RemoveCondition for each. RAW: 1 hour, no concentration. We install
    /// with a `Rounds(100)` timer so it lasts for any plausible encounter.
    /// Joins `is_dispellable_buff` so Dispel Magic can rip the buff cleanly.
    Footloose,
    /// Otherworldly-Guised (5e Tasha's Otherworldly Guise, level-6
    /// transmutation, concentration). The caster shifts into a
    /// celestial-style form: they gain +2 AC (read by
    /// `condition_ac_bonus`), resistance to radiant and poison damage
    /// (folded into `TYPED_RESISTANCE_CONDITIONS`), the Flying speed bump
    /// (joins the `Flying` cohort in `is_flying` / `condition_speed_bonus`),
    /// dynamic immunity to Charmed / Frightened (`dynamic_immunity_to`
    /// chokepoint), and every melee weapon hit deals +2d6 radiant via the
    /// on_hit_riders table. Concentration-bound on the caster; dropping
    /// concentration drops the buff. Joins `is_dispellable_buff` so Dispel
    /// Magic / Counterspell can rip it.
    OtherworldlyGuised,
    /// Wreathed in shadows (5e XGE Shadow of Moil, level-4 warlock evocation,
    /// concentration). The caster is enveloped by clinging darkness: any
    /// creature that hits them with a melee attack takes 2d8 necrotic damage
    /// in retaliation (the reflect rider lives in `MELEE_REFLECT_RIDERS`
    /// alongside Fire Shield / Armor of Agathys / Investiture of Flame), AND
    /// attacks against them have disadvantage (the holder is dim — joins the
    /// `imposes_disadvantage_to_attackers` cohort). RAW: the caster also
    /// sheds dim light but is otherwise illuminated normally to themselves.
    /// We collapse the lighting clause since the engine has no sight system.
    /// Concentration-bound on the caster.
    MoilShrouded,
    /// Elementally-Weaponed (5e Elemental Weapon, level-3 transmutation,
    /// concentration). The target's weapon is sheathed in elemental energy:
    /// +1 attack and +1d4 fire damage on every melee weapon hit. The +1
    /// attack-roll bump rides the standard `attack_bonus_buff` lane (paired
    /// with the concentration's `with_attack_buffs` ledger so dropping the
    /// spell rolls back the bump); the +1d4 fire per-hit rider lives in the
    /// `ON_HIT_RIDERS` table next to Spirit Shroud / Crusader's Mantle —
    /// persistent (non-consumed), melee-only. RAW lets the caster pick the
    /// element (acid / cold / fire / lightning / thunder); we collapse to
    /// fire as the flavor default since the engine's per-cast picker UI
    /// doesn't yet surface a damage-type selection. Cleared on concentration
    /// drop via the standard `Condition` cleanup hook + the `attack_buffs`
    /// rollback in `drop_concentration`. Joins `is_dispellable_buff` so
    /// Dispel Magic / Counterspell can rip it.
    ElementallyWeaponed,
    /// True-Sighted (5e True Seeing, level-6 divination). The holder
    /// perceives things as they actually are: invisible creatures, magical
    /// blur, and displacement illusions all stop hiding the truth from
    /// them. The mechanical envelope: when the holder is the *attacker*,
    /// they ignore the disadvantage their target's `Invisible` / `Blurred`
    /// / `Displaced` would otherwise impose (gated via the
    /// `countered_by_truesight` cohort in `compute_attack_mode`). When the
    /// holder is the *target*, an attacker can't ride the matching
    /// advantage from their own `Invisible` (same gate, on the
    /// attacker-side advantage lane). Concentration-free RAW (1 hour);
    /// installed as a flat `Rounds` timer plus the standard
    /// `is_dispellable_buff` hook so Dispel Magic can strip it. Distinct
    /// from `Blindsight` template senses — those are creature-intrinsic
    /// (no condition) and apply unconditionally; True Sight is a
    /// short-duration *buff* a caster can toggle for a specific fight.
    TrueSighted,
    /// Seeing-Invisible (5e See Invisibility, level-2 divination). The
    /// strictly weaker sibling of `TrueSighted` and the reason the
    /// engine's concealment piercing is tiered rather than boolean: the
    /// holder sees invisible creatures, and *only* invisible creatures.
    /// A Blurred or Displaced target still fools them completely — RAW
    /// those aren't invisibility, they distort where you appear to be,
    /// which is a lie a viewer who can see you still believes.
    ///
    /// Mechanically: `concealment_piercing_of` reports
    /// `ConcealmentPiercing::Invisibility` for the holder, which
    /// suppresses exactly the `Invisible` row of the attack-mode
    /// concealment clauses (both polarities — the holder ignores an
    /// Invisible target's disadvantage, and an Invisible attacker gets
    /// no advantage against the holder) and lifts the holder's
    /// `viewer_can_see` gate against an Invisible subject.
    ///
    /// Concentration-free RAW (1 hour); installed as a flat `Rounds`
    /// timer plus the standard `is_dispellable_buff` hook so Dispel
    /// Magic can strip it, matching `TrueSighted` exactly on both.
    SeeingInvisible,
    /// Immolated (5e Immolation, level-5 transmutation, concentration).
    /// The target is engulfed in magical flames: they take 4d6 fire damage
    /// at the end of each of their turns (via the standard
    /// `ROUND_END_DOTS` registry — entry sits next to Burning / Witch Bolt
    /// in the round-end drip). They can shake off the spell with a DEX
    /// save at end-of-turn (via the standard `ROUND_END_SAVES` registry —
    /// entry sits next to Hold Person / Hideous Laughter). Concentration-
    /// bound on the caster; dropping concentration extinguishes the flames
    /// cleanly. Distinct from `Burning` (Searing Smite / Fire Bolt
    /// ignition): Immolated is a much fatter drip (4d6 vs 1d4) and
    /// concentration-anchored rather than timer-only, matching the lv5
    /// spell-slot cost.
    Immolated,
    /// Guided Strike primed (5e War Domain Cleric Channel Divinity, lv2
    /// subclass feature). The cleric has spent their once-per-short-rest
    /// Channel Divinity charge to gain a flat +10 bonus to their next
    /// attack roll — the largest single-swing accuracy buff in the game,
    /// meant to turn a marginal near-miss into a guaranteed connect for a
    /// smite / rider payoff. Read by `condition_attack_bonus` (+10) and
    /// consumed by `CONSUMED_ON_ATTACK` on the first swing that fires
    /// after the prime installs. Sits in the same one-shot-attack-buff
    /// lane as `Inspired` / `PrecisionAttacking` / `TidesOfChaos` — all
    /// self-installed primes that burn on the next attack roll. Tick-down
    /// timer (`UntilStartOfNextTurn`) caps an unused prime so the buff
    /// doesn't dangle across rounds when the swing whiffs the reach
    /// window.
    GuidedStriking,
    /// Marked for the Grave (5e Grave Domain Cleric Channel Divinity,
    /// lv2 subclass — Path to the Grave, XGtE). The cleric has cursed
    /// this target for a killing blow. RAW: "the next time you or an
    /// ally of yours hits the cursed creature with an attack, the
    /// creature has vulnerability to all of that attack's damage, and
    /// then the curse ends."
    ///
    /// All three clauses ship:
    ///
    ///   - **Advantage** on attacks against the holder, via the shared
    ///     `grants_advantage_to_attackers` cohort next to
    ///     `GuidingBoltLit` / `Outlined`. (Strictly a house addition —
    ///     RAW grants no advantage — kept because it is what makes the
    ///     curse worth a Channel Divinity in a fight the cleric might
    ///     not otherwise land a hit in.)
    ///   - **Vulnerability to every damage type**, via the
    ///     `TYPED_VULNERABILITY_CONDITIONS` cohort, which
    ///     `effective_damage` reads alongside the target's own
    ///     `damage_modifiers`. "All of that attack's damage" is why the
    ///     row lists `DamageType::ALL`: the curse does not care what
    ///     the blow was made of, and a swing that lands piercing plus
    ///     radiant plus necrotic doubles all three.
    ///   - **"And then the curse ends"**, via
    ///     `VULNERABILITIES_SPENT_BY_THE_ATTACK`, which both attack
    ///     chokepoints queue a removal for behind the swing's last
    ///     damage effect. The `UntilStartOfNextTurn` timer is the
    ///     backstop for a curse nobody cashes in.
    ///
    /// Distinct from `GuidingBoltLit` semantically (Cleric CD curse
    /// vs. spell-mark) even though they overlap on the next-attack-
    /// advantage mechanic — kept as a separate condition so combat log
    /// lines identify the curse source, and so the vulnerability half
    /// attaches here without touching Guiding Bolt's lane.
    MarkedForGrave,
    /// Power Word Pain (5e level-7 necromancy, XGtE). The target's body
    /// is racked with excruciating pain: attack rolls suffer
    /// disadvantage (joins `imposes_attacker_disadvantage`) and the
    /// walking speed halves (joins the shared `SPEED_MULTIPLIERS`
    /// multiplicative table next to Hasted / Slowed). Concentration-
    /// bound on the caster. At the end of each of the target's turns
    /// the target can attempt a CON save vs the caster's spell DC to
    /// shake the pain off — registered on the shared `ROUND_END_SAVES`
    /// table next to Hold Person's WIS-vs-Stunned save and Flesh to
    /// Stone's CON-vs-Petrified save (same save-then-clear-and-drop-
    /// concentration semantics, distinct save/condition axis). Distinct
    /// from `Slowed` (5e Slow spell): Slowed layers -2 AC and -2 DEX
    /// saves on top of the half-speed multiplier which don't fit RAW's
    /// Power Word Pain envelope — the pain condition rides the pure
    /// attack-disadvantage + half-speed lane so the target still saves
    /// against DEX-anchored bursts (Fireball, Lightning Bolt) at full
    /// bonus and doesn't lose AC. The HP-threshold gate (≤100 HP) is
    /// applied at spell cast time in `PowerWordPain::side_effects`
    /// (RAW's "you must have a target with 100 HP or less" clause),
    /// matching the same at-cast gate `PowerWordStun` uses for its
    /// ≤150 HP threshold and `PowerWordKill` uses for its ≤100 HP
    /// insta-kill. Not on `is_dispellable_buff` — it's a debuff on the
    /// enemy, not a friendly buff.
    PowerWordPained,
    /// Overchannel primed (5e Evocation Wizard, subclass level 14). The
    /// evoker has declared that their next damaging spell of level 1-5
    /// will deal **maximum** damage instead of rolling. Consumed at the
    /// shared spell-damage chokepoint (`roll_empowered_sum`), which
    /// swaps the roll for `Dice::max_roll` and latches the backlash the
    /// post-cast trigger then charges.
    ///
    /// Modeled as a prime because the engine has no "choose an option
    /// while casting" surface — the same shape the six sorcerer
    /// metamagic primes already use, minus the sorcery-point cost
    /// (Overchannel is free per RAW; its price is paid in necrotic
    /// backlash on every use after the first). Tick-down timer
    /// (`UntilStartOfNextTurn`) caps an unused prime so it doesn't sit
    /// across rounds, matching the metamagic primes.
    ///
    /// Not on `is_dispellable_buff`: it's a declaration about the
    /// caster's next cast, not a magical effect on them.
    Overchanneling,
    /// Marked by **Hexblade's Curse** (5e Hexblade Warlock, subclass
    /// level 1). A back-linked condition: the flag says the creature is
    /// cursed, and the link says by whom, which is load-bearing here
    /// because every one of the curse's three clauses is scoped to the
    /// hexblade personally.
    ///
    ///   - The hexblade adds their proficiency bonus to damage rolls
    ///     against the cursed target (`curse_damage_bonus`, folded in at
    ///     both attack-damage chokepoints).
    ///   - The hexblade's attack rolls against the cursed target crit on
    ///     19-20 (`crit_threshold_against`).
    ///   - If the cursed target drops, the hexblade regains
    ///     `warlock level + CHA mod` hit points (the curse-death row on
    ///     the shared drop-reward chokepoint).
    ///
    /// A second warlock's curse on the same creature takes over the
    /// link, which is the right reading of RAW — the curse is a single
    /// relationship between one hexblade and one target, and the engine
    /// stores exactly one link per condition.
    ///
    /// Ten-round timer (1 minute RAW). Not on `is_dispellable_buff` —
    /// it's a debuff on the enemy rather than a friendly buff.
    HexbladeCursed,
    /// **Symbiotic Entity** (5e Circle of Spores Druid, subclass level
    /// 2). The druid animates their spore halo into a symbiote that
    /// rides their body: they gain a slab of temporary hit points and,
    /// while the temp HP lasts, every melee weapon hit they land deals
    /// an extra 1d6 necrotic and Halo of Spores rolls its damage die
    /// twice.
    ///
    /// The two riders read the flag from opposite ends of the engine —
    /// the melee bump is an `ON_HIT_RIDERS` row keyed on this condition
    /// (persistent, `melee_only`), and the halo's double-die is a
    /// branch inside the halo action itself. What ties them together is
    /// RAW's duration clause: the feature "lasts for 10 minutes, until
    /// you lose all these temporary hit points, or until you use your
    /// Wild Shape again." We enforce the load-bearing half of that at
    /// the temp-HP chokepoint — `ActorInstance::take_damage` drops the
    /// condition on the tick the temp-HP pool empties — so the
    /// symbiote's lifetime is genuinely tied to the shield it came
    /// with, rather than to a timer that could outlive it.
    ///
    /// On `is_dispellable_buff`: it's a magical effect the druid put on
    /// themselves, and the same reasoning that puts Investiture of Ice
    /// and Barkskin on that list applies here.
    SymbioticEntity,
    /// **Ancestral Protectors** (5e Path of the Ancestral Guardian
    /// Barbarian, subclass level 3). A back-linked debuff: the flag
    /// says the creature is haunted by the barbarian's ancestors, and
    /// the link says which barbarian, which is load-bearing because
    /// both of the feature's clauses are scoped relative to that one
    /// creature.
    ///
    ///   - The haunted creature has disadvantage on attack rolls
    ///     against anyone *other than* the barbarian
    ///     (`compute_attack_mode` reads the link and compares it to the
    ///     defender).
    ///   - Any damage it deals to a creature other than the barbarian
    ///     is halved (`DealDamage::apply` reads the same link).
    ///
    /// The pairing is the whole feature: it doesn't stop the target
    /// hitting the barbarian, it makes hitting anyone else a bad
    /// trade. Marking a second creature moves the mark — RAW installs
    /// it on "the first creature you hit" each turn, and the engine
    /// stores one link per condition, so the newest mark wins in both.
    ///
    /// `UntilStartOfNextTurn` on the barbarian's own clock would be
    /// wrong (the mark is meant to survive the target's next turn, which
    /// is when it matters), so it carries a short `Rounds` timer
    /// instead. Not on `is_dispellable_buff` — it's a debuff.
    AncestrallyHaunted,
    /// **Bladesong** (5e Bladesinging Wizard, subclass level 2). The
    /// wizard's martial trance: while it is up they add their
    /// Intelligence modifier to AC, move 10 ft faster, and add the same
    /// modifier to Constitution saves made to maintain concentration.
    ///
    /// Three separate lanes read the one flag —
    /// `condition_ac_bonus` (the AC row is INT-scaled, so it sits in
    /// the scaling arm next to Sacred rather than in the flat table),
    /// `condition_speed_bonus`, and `roll_concentration_save` — which
    /// is what makes Bladesong the wizard's answer to standing in melee
    /// at all: it patches the AC that lets them survive the swing and
    /// the concentration save that decides whether the spell they are
    /// holding survives it too.
    ///
    /// Ten-round timer (1 minute RAW). On `is_dispellable_buff`.
    Bladesinging,
    /// **Form of Dread** (5e Undead Warlock, subclass level 1). The
    /// warlock takes on the shape of the thing their patron is: they
    /// gain temporary hit points, they cannot be Frightened, and once
    /// on each of their turns a creature they hit must make a Wisdom
    /// save or be Frightened of them.
    ///
    /// Three clauses, three lanes, and the arrangement is what makes it
    /// coherent: the fear immunity (`CONDITION_DRIVEN_IMMUNITIES`) and
    /// the fear rider (`ON_HIT_RIDERS`, with a once-per-turn key and a
    /// save-gated follow-up) are the same coin. A warlock who is
    /// handing out Frightened wants to be the one creature on the field
    /// it cannot be handed back to.
    ///
    /// Ten-round timer (1 minute RAW) rather than a temp-HP-bound one:
    /// RAW's Form of Dread grants temp HP but does *not* end when they
    /// run out, unlike the Spores Druid's Symbiotic Entity.
    ///
    /// On `is_dispellable_buff`.
    FormOfDread,
    /// **Kensei's Shot** (5e Way of the Kensei Monk, subclass level 3).
    /// A bonus action spent readying the bow: until the end of the
    /// turn, every ranged weapon attack the monk lands carries an extra
    /// 1d4.
    ///
    /// The engine's only `RiderLane::RangedWeapon` prime that isn't a
    /// spell, and the reason the monk chassis has a reason to hold a
    /// bow at all. Everything else on the sheet — Flurry of Blows,
    /// Stunning Strike, Martial Arts — pays the monk for standing in
    /// contact; this pays them for not.
    ///
    /// `UntilStartOfNextTurn` rather than a round count, because RAW's
    /// window is "until the end of the current turn" and the timer that
    /// clears on the holder's own next turn-start is exactly that.
    ///
    /// Not on `is_dispellable_buff` — like the maneuver primes, it's a
    /// declaration about the holder's next shot rather than a magical
    /// effect sitting on them.
    KenseisShot,
    /// Speed pinned to 0 by an outside effect that isn't a grapple, a
    /// restraint or a loss of consciousness. Installed today by the
    /// Conquest Paladin's Aura of Conquest, on a Frightened creature
    /// that starts its turn inside the aura.
    ///
    /// A condition rather than a direct drain on the movement budget,
    /// because RAW's clause is "its speed is 0, and it can't benefit
    /// from any bonus to its speed" — and the second half is the one
    /// that decides whether the aura actually holds anyone. Emptying
    /// the budget at turn-start would let the rooted creature Dash out
    /// of the aura a moment later; riding `zeros_movement` means the
    /// Dash hands them a budget that `remaining_movement` still reads
    /// as zero, which is exactly what RAW describes.
    ///
    /// `UntilStartOfNextTurn`, re-checked and re-installed every turn
    /// the aura still catches them, so walking free the moment the
    /// paladin drops or the fear lifts needs no teardown of its own.
    Rooted,
    /// 5e College of Eloquence Bard **Unsettling Words** — the bard
    /// picks a creature apart with a barbed remark, and the creature
    /// subtracts a Bardic Inspiration die from its next saving throw.
    ///
    /// The mirror image of `Inspired`, and deliberately built out of
    /// the same three pieces: a flat magnitude on the shared
    /// `CONDITION_SAVE_BONUSES` cohort (−4, the d8 average floored,
    /// matching the +4 Precision Attack already collapses a d8 to), a
    /// short timer, and a row on `CONSUMED_ON_SAVE` so the penalty
    /// spends itself on the first save the holder rolls rather than
    /// riding every save until the timer runs out.
    ///
    /// `Rounds(2)` rather than `UntilStartOfNextTurn`: RAW's window is
    /// "before the end of your next turn", and a target who saves
    /// against nothing in the intervening round should still be
    /// carrying the quip when the bard's Hold Person lands on their
    /// own turn. The consume-on-save row is what stops the longer
    /// timer from over-granting.
    Unsettled,
    /// 5e Circle of Stars Druid **Starry Form: Archer** (subclass level
    /// 2). The druid takes on the shape of a constellation and gains a
    /// bonus-action ranged attack — `STARRY_BOLT`, a 1d8 + WIS radiant
    /// spell attack out to 60 ft — for as long as the form holds.
    ///
    /// The condition carries no passive modifier of its own; it is
    /// purely the gate the bolt's `custom_validate_input` reads. That
    /// is the whole shape of the Archer: one extra button, available
    /// every round, on a chassis whose bonus action was otherwise
    /// spent on Healing Word or nothing at all.
    ///
    /// Mutually exclusive with its two siblings — see
    /// `STARRY_FORMS`, which the install path walks to strip the other
    /// two before applying this one.
    StarryFormArcher,
    /// 5e Circle of Stars Druid **Starry Form: Chalice** (subclass
    /// level 2). While the form holds, every slot-cast heal the druid
    /// lands spills over: a second wounded ally within 30 ft regains
    /// 1d8 + WIS on top of whatever the spell itself restored.
    ///
    /// Read at the shared slot-heal chokepoint
    /// (`starry_chalice_overflow`), which is why Cure Wounds, Healing
    /// Word and Mass Cure Wounds all pick it up without three separate
    /// riders. Like `StarryFormArcher` it stores no magnitude — the
    /// die and the ability modifier are re-derived at the heal site
    /// from the druid's own Wisdom.
    StarryFormChalice,
    /// 5e Circle of Stars Druid **Starry Form: Dragon** (subclass level
    /// 2). While the form holds, a d20 of 9 or lower on a
    /// concentration save counts as a 10.
    ///
    /// The only one of the three forms that reaches into the dice
    /// rather than into the action economy, and the reason
    /// `roll_save_with_extra_mode_and_bonus` grew a `d20_floor`
    /// parameter. On a WIS-18 druid with CON +2 and no save
    /// proficiency the floor turns a coin-flip on a DC 10
    /// concentration check into a near-certainty, which is what makes
    /// Dragon the form a druid picks when the thing they are holding
    /// up — Moonbeam, Call Lightning, Spike Growth — matters more than
    /// anything they could do with the round.
    StarryFormDragon,
    /// 5e Arcane Archer Fighter **Arcane Shot: Banishing Arrow**
    /// (subclass level 3). Nocked onto the bow; the next ranged weapon
    /// hit forces a CHA save or the target is swept into a harmless
    /// demiplane until the end of the archer's next turn.
    ///
    /// The six Arcane Shot primes are one family (`ARCANE_SHOTS`) and
    /// share one charge pool, so nocking a second one strips the first
    /// — see `ExclusivePrime`. Every one of them rides
    /// `RiderLane::RangedWeapon`: RAW is "when you fire an arrow from a
    /// shortbow or longbow", and an archer who drew a sword would be
    /// firing nothing.
    ArcaneShotBanishing,
    /// 5e Arcane Archer Fighter **Arcane Shot: Beguiling Arrow**
    /// (subclass level 3). 2d6 psychic on the hit, then a CHA save or
    /// the target is Charmed.
    ///
    /// RAW charms the target of an *ally* the archer chooses, which the
    /// engine has no way to nominate at rider-resolution time; the charm
    /// links back to the archer instead. That is the narrower reading —
    /// it takes the target out of the archer's own fight rather than
    /// someone else's — and it is the one the `Charmed` back-link
    /// machinery already enforces end to end.
    ArcaneShotBeguiling,
    /// 5e Arcane Archer Fighter **Arcane Shot: Bursting Arrow**
    /// (subclass level 3). The arrow detonates: 2d6 force to everything
    /// standing near the creature it struck, with no save.
    ///
    /// The only Arcane Shot whose payload is not aimed at the creature
    /// hit, and the reason `FollowUpEffect` grew a `Burst` variant —
    /// `Splash` picks one adjacent enemy, and a detonation that hit one
    /// bystander out of four would be a different feature.
    ArcaneShotBursting,
    /// 5e Arcane Archer Fighter **Arcane Shot: Enfeebling Arrow**
    /// (subclass level 3). 2d6 necrotic on the hit, then a CON save or
    /// the target's own weapon damage is halved until the end of the
    /// archer's next turn — the `Enfeebled` condition below.
    ArcaneShotEnfeebling,
    /// 5e Arcane Archer Fighter **Arcane Shot: Grasping Arrow**
    /// (subclass level 3). 2d6 poison on the hit, then a STR save or
    /// brambles erupt and hold the target fast (Restrained).
    ///
    /// RAW's ongoing 2d6 slashing per turn of struggle collapses into
    /// the Restrained condition alone, which is where the tactical
    /// weight of the shot sits: speed zero, disadvantage on its own
    /// attacks, advantage for everyone shooting at it.
    ArcaneShotGrasping,
    /// 5e Arcane Archer Fighter **Arcane Shot: Shadow Arrow**
    /// (subclass level 3). 2d6 psychic on the hit, then a WIS save or
    /// the target can see nothing beyond 5 ft.
    ///
    /// Modeled as Blinded, which is the honest collapse of "can't see
    /// beyond 5 feet" on a grid where the archer is never inside 5 feet
    /// of the thing they just shot: from the target's side, everything
    /// worth attacking has gone dark.
    ArcaneShotShadow,
    /// Weapon damage dealt by this creature is halved. Installed by the
    /// Arcane Archer's Enfeebling Arrow on a failed CON save.
    ///
    /// The mirror image of `DamageResistant`, which halves damage on the
    /// way *in*; this halves it on the way *out*, and it is read at the
    /// attacker-scoped site in `attack::attacker_scoped_damage_reduction`
    /// rather than at the victim's. Weapon attacks only, per RAW — a
    /// spellcaster shrugs the arrow's necrotic drain off entirely, which
    /// is the trade the shot makes for how hard it hits a multiattacker.
    Enfeebled,
    /// 5e Way of the Astral Self Monk **Arms of the Astral Self**
    /// (subclass level 3, TCE). Spectral arms of ki settle over the
    /// monk's own, and for as long as they hold the monk punches with
    /// something that is not quite a fist: Wisdom to hit and to damage,
    /// force instead of bludgeoning, and five more feet of reach.
    ///
    /// The condition stores none of that. It is the gate that
    /// `ASTRAL_ARMS_STRIKE` — a `SimpleWeapon` carrying all four
    /// differences as ordinary weapon data — reads through
    /// `SimpleWeapon::requires_condition`, which is the same shape the
    /// Stars Druid's `StarryFormArcher` uses to gate `STARRY_BOLT`: the
    /// form is a condition, the thing the form lets you do is an action
    /// that refuses to validate without it.
    ///
    /// Also read on the caster side of `ONCE_PER_TURN_WEAPON_DIE_RIDERS`
    /// by Empowered Arms (lv11), which is why that cohort grew a
    /// `caster_gate`: the extra martial-arts die is a property of the
    /// arms being up, not of the monk carrying the feature.
    AstralArms,
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
            Condition::MarkedForGrave => "marked for the grave",
            Condition::PowerWordPained => "racked with power word pain",
            Condition::Overchanneling => "overchanneling",
            Condition::Helped => "helped",
            Condition::Hidden => "hidden",
            Condition::Burning => "burning",
            Condition::Readied => "readied",
            Condition::Disengaging => "disengaging",
            Condition::Unconscious => "unconscious",
            Condition::Baned => "baned",
            Condition::Deafened => "deafened",
            Condition::HuntersMarked => "marked by hunter's mark",
            Condition::MageArmored => "mage armored",
            Condition::Aided => "aided",
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
            // Source-neutral wording: the condition is installed both by
            // the Compelled Duel spell and by the Cavalier's Unwavering
            // Mark, so "compelled to duel" would misdescribe half its
            // holders.
            Condition::Dueled => "locked into a duel",
            Condition::SearingSmiting => "primed to sear",
            Condition::WrathfulSmiting => "primed with wrath",
            Condition::BrandingSmiting => "primed to brand",
            Condition::BlindingSmiting => "primed to blind",
            Condition::HeatMetaled => "burning from heat metal",
            Condition::StunningStrike => "primed to stun",
            Condition::Inspired => "inspired",
            Condition::Exhausted => "exhausted",
            Condition::SpiritShrouded => "wreathed in spirits",
            Condition::SymbioticEntity => "bonded to a spore symbiote",
            Condition::AncestrallyHaunted => "haunted by ancestral spirits",
            Condition::Bladesinging => "bladesinging",
            Condition::FormOfDread => "wearing a form of dread",
            Condition::KenseisShot => "drawing a kensei shot",
            Condition::Rooted => "rooted in place",
            Condition::Unsettled => "unsettled",
            Condition::HolyAuraed => "haloed in holy light",
            Condition::Foreseen => "foreseen",
            Condition::Confused => "confused",
            Condition::Entangled => "entangled",
            Condition::Flying => "flying",
            Condition::SpiderClimbing => "spider-climbing",
            Condition::Dominated => "dominated",
            Condition::Feebled => "feebleminded",
            Condition::Dancing => "dancing helplessly",
            Condition::Mazed => "trapped in a maze",
            Condition::EyebittenSick => "afflicted by eyebite",
            Condition::Conjured => "conjured",
            Condition::AgathysShielded => "armored in agathys",
            Condition::BigbysHanded => "guarded by bigby's hand",
            Condition::Transformed => "transformed",
            Condition::Duplicity => "shadowed by an illusory double",
            Condition::Summoner => "attended by summoned allies",
            Condition::DivineStriking => "primed with divine strike",
            Condition::DivineStrikingPoison => "primed with envenomed divine strike",
            Condition::FangsOfTheFireSnake => "wreathed in the fire snake's fangs",
            Condition::TripAttacking => "primed to trip",
            Condition::InvestedInFlame => "invested with flame",
            Condition::InvestedInIce => "invested with ice",
            Condition::InvestedInStone => "invested with stone",
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
            Condition::Reduced => "reduced",
            Condition::GiantsMight => "wreathed in giant's might",
            Condition::FireRuneInvoked => "burning with a fire rune",
            Condition::TouchingDeath => "wreathed in the reaper's touch",
            Condition::DivineStrikingNecrotic => "primed to strike withering",
            Condition::DivineStrikingPsychic => "primed to strike a verdict",
            Condition::WardingBonded => "bonded by warding bond",
            Condition::MindBlanked => "mind-blanked",
            Condition::LightningArrowPrimed => "primed with lightning arrow",
            Condition::EnsnaringStriking => "primed with ensnaring strike",
            Condition::ZephyrStriking => "primed with zephyr strike",
            Condition::Barkskinned => "barkskinned",
            Condition::Untracked => "passing without trace",
            Condition::HolyWeaponed => "wielding a holy weapon",
            Condition::Displaced => "displaced",
            Condition::AbsorbedElements => "absorbing elements",
            Condition::DangerSense => "sensing danger",
            Condition::WitchBolted => "tethered by witch bolt",
            Condition::SpiritGuarding => "guarded by spirits",
            Condition::MenacingAttacking => "primed to menace",
            Condition::DisarmingAttacking => "primed to disarm",
            Condition::PushingAttacking => "primed to push",
            Condition::Disarmed => "disarmed",
            Condition::GoadingAttacking => "primed to goad",
            Condition::Goaded => "goaded",
            Condition::PrecisionAttacking => "primed for a precision strike",
            Condition::SweepingAttacking => "primed for a sweeping strike",
            Condition::LungingAttacking => "primed to lunge",
            Condition::EmpoweredSpelling => "primed with empowered spell",
            Condition::HeightenedSpelling => "primed with heightened spell",
            Condition::CarefulSpelling => "primed with careful spell",
            Condition::DistantSpelling => "primed with distant spell",
            Condition::TwinnedSpelling => "primed with twinned spell",
            Condition::ExtendedSpelling => "primed with extended spell",
            Condition::SeekingSpelling => "primed with seeking spell",
            Condition::SubtleSpelling => "primed with subtle spell",
            Condition::TidesOfChaos => "riding the tides of chaos",
            Condition::StarryFormArcher => "starry form (archer)",
            Condition::StarryFormChalice => "starry form (chalice)",
            Condition::StarryFormDragon => "starry form (dragon)",
            Condition::ArcaneShotBanishing => "banishing arrow nocked",
            Condition::ArcaneShotBeguiling => "beguiling arrow nocked",
            Condition::ArcaneShotBursting => "bursting arrow nocked",
            Condition::ArcaneShotEnfeebling => "enfeebling arrow nocked",
            Condition::ArcaneShotGrasping => "grasping arrow nocked",
            Condition::ArcaneShotShadow => "shadow arrow nocked",
            Condition::Enfeebled => "enfeebled",
            Condition::AstralArms => "arms of the astral self",
            Condition::TransmutedSpelling => "primed with transmuted spell",
            Condition::CunningStrikePoison => "primed with cunning poison",
            Condition::CunningStrikeTrip => "primed with cunning trip",
            Condition::CunningStrikeWithdraw => "primed with cunning withdraw",
            Condition::CunningStrikeDaze => "primed with cunning daze",
            Condition::CausticBrewed => "splashed with caustic brew",
            Condition::DistractingAttacking => "primed to distract",
            Condition::Distracted => "distracted",
            Condition::EldritchStruck => "eldritch-struck",
            Condition::WarMagicPrimed => "primed with war magic",
            Condition::Shadowstepping => "stepping through shadow",
            Condition::WildShaped => "wild-shaped",
            Condition::Sworn => "sworn-quarry of a vengeance paladin",
            Condition::Purified => "purified",
            Condition::PhantasmalForced => "haunted by a phantasm",
            Condition::WaterSphered => "trapped in a watery sphere",
            Condition::InvestedInWind => "invested with wind",
            Condition::WindBlasted => "wind-blasted",
            Condition::Longstriding => "longstriding",
            Condition::Coiled => "coiled by a tentacle",
            Condition::ExpeditiouslyRetreating => "expeditiously retreating",
            Condition::FlamingArrowed => "wielding flame arrows",
            Condition::AshardalonStriding => "striding with elemental power",
            Condition::OtherworldlyGuised => "guised in otherworldly form",
            Condition::Silenced => "silenced",
            Condition::Footloose => "moving freely",
            Condition::MoilShrouded => "shrouded in moil",
            Condition::ElementallyWeaponed => "wielding an elemental weapon",
            Condition::TrueSighted => "true-sighted",
            Condition::SeeingInvisible => "seeing-invisible",
            Condition::Immolated => "immolated",
            Condition::GuidedStriking => "primed with a guided strike",
            Condition::HexbladeCursed => "cursed by a hexblade",
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
        // Categorical primes funnel through their own helpers so adding
        // a new metamagic / smite / maneuver / cunning prime drops the
        // bookkeeping to a single list edit (the prime helper) and the
        // dispel sweep picks it up automatically.
        if self.is_metamagic_prime()
            || self.is_cunning_strike_prime()
            || self.is_smite_prime()
            || self.is_maneuver_prime()
        {
            return true;
        }
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
                | Condition::Sacred
                | Condition::Inspired
                | Condition::SpiritShrouded
                | Condition::SymbioticEntity
                | Condition::Bladesinging
                // 5e Circle of Stars Druid Starry Form, all three
                // shapes. Ten-round self-buffs bought with a
                // once-per-short-rest charge, which is exactly the
                // envelope Bladesong directly above already sits in —
                // and, like Bladesong, worth a Dispel Magic.
                | Condition::StarryFormArcher
                | Condition::StarryFormChalice
                | Condition::StarryFormDragon
                // 5e Way of the Astral Self Monk Arms of the Astral
                // Self. A ten-round self-buff on the same envelope as
                // the Starry Forms above it, and rather more worth a
                // Dispel Magic than they are: stripping the arms does
                // not just take a button away, it takes the monk's best
                // attack off the board entirely and drops them back to
                // a weaker punch at half the reach.
                | Condition::AstralArms
                | Condition::FormOfDread
                | Condition::HolyAuraed
                | Condition::Foreseen
                | Condition::Flying
                | Condition::SpiderClimbing
                | Condition::AgathysShielded
                | Condition::BigbysHanded
                | Condition::Transformed
                | Condition::InvestedInFlame
                | Condition::InvestedInIce
                | Condition::InvestedInStone
                | Condition::InvestedInWind
                | Condition::WindWalled
                | Condition::Shillelaghed
                | Condition::Enlarged
                | Condition::GiantsMight
                | Condition::WardingBonded
                | Condition::MindBlanked
                | Condition::Barkskinned
                | Condition::Untracked
                | Condition::HolyWeaponed
                | Condition::Displaced
                | Condition::AbsorbedElements
                | Condition::SpiritGuarding
                | Condition::TidesOfChaos
                | Condition::Purified
                | Condition::Longstriding
                | Condition::ExpeditiouslyRetreating
                | Condition::FlamingArrowed
                | Condition::AshardalonStriding
                | Condition::OtherworldlyGuised
                | Condition::Footloose
                | Condition::MoilShrouded
                | Condition::ElementallyWeaponed
                | Condition::TrueSighted
                | Condition::SeeingInvisible
                | Condition::GuidedStriking
        )
    }

    /// True if the holder's "true sight" buff (5e True Seeing) lets them
    /// see through this condition's concealment / illusion envelope. Read
    /// by `compute_attack_mode` to suppress:
    ///   * the disadvantage a target's `Invisible` / `Blurred` /
    ///     `Displaced` would otherwise impose on an attacker who is
    ///     true-sighted (attacker side),
    ///   * the advantage an attacker's own `Invisible` would otherwise
    ///     grant when their target is true-sighted (target side).
    ///
    /// Centralizes the cohort so adding a new "illusory / invisibility-
    /// style" concealment (e.g. a future Hide-in-Mists / Etherealness)
    /// drops to a one-line edit instead of two scattered `matches!`
    /// branches at the attack-mode site.
    pub fn countered_by_truesight(&self) -> bool {
        matches!(
            self,
            Condition::Invisible | Condition::Blurred | Condition::Displaced
        )
    }

    /// True if this condition's concealment is the **invisibility**
    /// kind specifically — the strictly narrower cohort that a See
    /// Invisibility effect pierces, as against the full
    /// `countered_by_truesight` set that Truesight pierces.
    ///
    /// RAW draws the line exactly here: See Invisibility (and the
    /// Divination Wizard's Third Eye, which grants it) lets you "see
    /// invisible creatures and objects" and nothing else. Blur and
    /// Displacement are not invisibility — they distort where you
    /// appear to be, which a viewer who can see you is still fooled by
    /// — so a See Invisibility holder attacking a Blurred target still
    /// swings at disadvantage.
    ///
    /// Every member of this cohort must also be in
    /// `countered_by_truesight`: Truesight is strictly stronger, and
    /// `ConcealmentPiercing`'s ordering encodes that. A drift test pins
    /// the containment.
    pub fn countered_by_see_invisibility(&self) -> bool {
        matches!(self, Condition::Invisible)
    }

    /// True if this condition is one of the **Smite-family** primes —
    /// the bonus-action "next weapon hit gets a damage / follow-up rider"
    /// buffs that consume from the caster on the next weapon swing.
    /// Spans three flavors on the same one-shot on-hit-rider corner:
    ///   - **Paladin melee smite spells**: `SearingSmiting` /
    ///     `WrathfulSmiting` / `BrandingSmiting` / `BlindingSmiting` /
    ///     `StaggeringSmiting` / `BanishingSmiting` / `ThunderousSmiting`
    ///     (via the shared `SmiteSpell` chassis + the paladin's
    ///     `ALL_SMITE_SPELLS` registry).
    ///   - **Ranger smite spells**: `LightningArrowPrimed` (lv3),
    ///     `EnsnaringStriking` (lv1), `ZephyrStriking` (lv1) — same
    ///     `SmiteSpell` chassis, different registry
    ///     (`ALL_RANGED_SMITE_SPELLS`) and AI engagement heuristic
    ///     (bow-range vs melee-adjacent).
    ///   - **Class-feature smites**: `Smiting` (Paladin Divine Smite)
    ///     and `DivineStriking` (Cleric Divine Strike / Twilight
    ///     Domain).
    ///     Centralized so the dispellable-buff sweep, the AI's "don't
    ///     double-prime" gates, and any future smite-aware chokepoint read
    ///     a single helper instead of listing each prime by name — adding
    ///     a new smite prime (a hypothetical 2024 paladin smite variant,
    ///     a future ranger smite pickup, etc.) lands as one row on this
    ///     matches! arm and the dispellable-buff sweep picks it up
    ///     automatically without a second edit on the OR list below.
    ///     Mutually exclusive in spirit (only one rider lands per swing
    ///     RAW; the engine doesn't enforce stacking — adding two primes
    ///     lets both ride).
    pub fn is_smite_prime(&self) -> bool {
        matches!(
            self,
            Condition::Smiting
                | Condition::SearingSmiting
                | Condition::WrathfulSmiting
                | Condition::BrandingSmiting
                | Condition::BlindingSmiting
                | Condition::StaggeringSmiting
                | Condition::BanishingSmiting
                | Condition::ThunderousSmiting
                | Condition::DivineStriking
                // Trickery Domain's poison-typed arm of the same
                // level-8 feature — same lane, same lifecycle, one
                // rider row apart.
                | Condition::DivineStrikingPoison
                | Condition::LightningArrowPrimed
                | Condition::EnsnaringStriking
                | Condition::ZephyrStriking
        )
    }

    /// True if this condition is one of the Fighter Battle Master
    /// **maneuver** primes — bonus-action "next melee hit gets a control
    /// rider" buffs (TripAttacking / MenacingAttacking / DisarmingAttacking
    /// / PushingAttacking / GoadingAttacking / PrecisionAttacking /
    /// SweepingAttacking / LungingAttacking / DistractingAttacking).
    /// Centralized so the dispellable-buff sweep and any future maneuver-
    /// aware chokepoint read a single helper instead of listing each
    /// prime by name. RAW: each maneuver is a separate superiority die
    /// spend; multiple primes can stack here since the rider-table consume
    /// site picks the first one that matches.
    pub fn is_maneuver_prime(&self) -> bool {
        matches!(
            self,
            Condition::TripAttacking
                | Condition::MenacingAttacking
                | Condition::DisarmingAttacking
                | Condition::PushingAttacking
                | Condition::GoadingAttacking
                | Condition::PrecisionAttacking
                | Condition::SweepingAttacking
                | Condition::LungingAttacking
                | Condition::DistractingAttacking
        )
    }

    /// True if this condition is one of the Sorcerer metamagic primes —
    /// the bonus-action "next spell does X" buffs (`Empowered` / `Heightened`
    /// / `Careful` / `Distant` / `Twinned` / `Extended` / `Seeking` /
    /// `Subtle`). Centralized so the AI's "don't double-prime" gates can
    /// read a single helper instead of listing each prime by name.
    /// Tides of Chaos is intentionally *excluded* — it's a 1/long-rest
    /// Wild Magic feature with no SP cost, so doubling up with a metamagic
    /// prime is RAW-legal and tactically sensible.
    pub fn is_metamagic_prime(&self) -> bool {
        matches!(
            self,
            Condition::EmpoweredSpelling
                | Condition::HeightenedSpelling
                | Condition::CarefulSpelling
                | Condition::DistantSpelling
                | Condition::TwinnedSpelling
                | Condition::ExtendedSpelling
                | Condition::SeekingSpelling
                | Condition::SubtleSpelling
                | Condition::TransmutedSpelling
        )
    }

    /// True if this condition is one of the 2024 Rogue **Cunning Strike**
    /// primes (`Poison` / `Trip` / `Withdraw` / `Daze`). Centralized so
    /// the bonus-action validators, the shortsword consume site, and the
    /// AI "don't double-prime" gate all read a single helper instead of
    /// open-coding the four-way `||` check. Mutually exclusive RAW (and
    /// in this engine): only one cunning prime is installed at a time.
    pub fn is_cunning_strike_prime(&self) -> bool {
        matches!(
            self,
            Condition::CunningStrikePoison
                | Condition::CunningStrikeTrip
                | Condition::CunningStrikeWithdraw
                | Condition::CunningStrikeDaze
        )
    }

    /// True if this condition zeros out movement. Read by
    /// `ActorInstance::remaining_movement` to gate motion-blocking
    /// conditions in one place.
    pub fn zeros_movement(&self) -> bool {
        // Note: Prone is NOT in this list. RAW: prone halves movement
        // (you crawl). The half-speed reduction is modelled in
        // `ActorInstance::remaining_movement()`, and standing up costs
        // half the actor's speed via `StandUp::cost()`. Blocking
        // movement entirely would create a catch-22 — standing up pays
        // in movement.
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
                | Condition::Rooted
                | Condition::WaterSphered
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
                // Exhausted is deliberately absent: the flag now means
                // "at least tier 1", and RAW's attack-roll penalty does
                // not arrive until tier 3. The gate lives in
                // `compute_attack_mode`, which can read the tier.
                | Condition::Confused
                | Condition::Dominated
                | Condition::Feebled
                | Condition::Dancing
                | Condition::MentallyImprisoned
                | Condition::Sphered
                | Condition::EarthenGrasped
                | Condition::Disarmed
                | Condition::WaterSphered
                | Condition::PowerWordPained
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
                | Condition::TidesOfChaos
                // 5e Trickery Domain Cleric **Invoke Duplicity** — the
                // illusory double's whole combat surface. Persistent
                // (deliberately off `CONSUMED_ON_ATTACK`) so it rides
                // every swing for its ten rounds, unlike the one-shot
                // `TidesOfChaos` / `Helped` / `Hidden` primes above it.
                | Condition::Duplicity
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
                | Condition::Asleep
                | Condition::Outlined
                | Condition::Petrified
                | Condition::GuidingBoltLit
                // 5e Grave Domain Cleric **Path to the Grave** Channel
                // Divinity (lv2 subclass, XGtE) — cursed target grants
                // advantage on the next attack. Sibling arm to
                // `GuidingBoltLit` on the "target-side advantage rider
                // with short-timer decay" lane; kept as a distinct
                // condition so the combat-log identity ("marked for the
                // grave" vs "marked by guiding bolt") disambiguates the
                // curse source at the log site.
                | Condition::MarkedForGrave
                | Condition::Dancing
                | Condition::MentallyImprisoned
                | Condition::Sphered
                | Condition::EarthenGrasped
                | Condition::Lifted
                | Condition::WaterSphered
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
                | Condition::MoilShrouded
        )
    }

    /// True if *ranged* attacks targeting the holder get disadvantage but
    /// melee attacks are unaffected. The 5e Wind Wall clause is the
    /// canonical case — arrows / bolts deflect, swords don't. Read by
    /// `compute_attack_mode` only when `!is_melee` so a wind-walled
    /// caster still eats melee damage normally.
    pub fn imposes_disadvantage_to_ranged_attackers(&self) -> bool {
        matches!(self, Condition::WindWalled | Condition::InvestedInWind)
    }

    /// True if the *holder's own* ranged attacks roll with disadvantage,
    /// while their melee swings are unaffected. Storm Sphere's residual
    /// gusts are the canonical case — bow / spell-arrow shots get
    /// thrown off, but melee weapons still bite normally. Symmetric to
    /// `imposes_disadvantage_to_ranged_attackers` (which imposes the
    /// penalty on attackers shooting *into* the holder), but on the
    /// attacker side. Read by `compute_attack_mode` only when
    /// `!is_melee` — gated on the ranged lane.
    pub fn imposes_attacker_disadvantage_on_ranged(&self) -> bool {
        matches!(self, Condition::WindBlasted)
    }

    /// True if the *holder's own* melee swings roll with advantage,
    /// while their ranged attacks are unaffected. The Shadow Monk's
    /// Shadow Step is the canonical case — RAW grants advantage on "the
    /// first melee attack you make before the end of the turn", so a
    /// shadow-stepped monk who reaches for a thrown weapon instead
    /// shouldn't collect the buff.
    ///
    /// Melee-side sibling of `imposes_attacker_disadvantage_on_ranged`
    /// (same attacker side, opposite lane and opposite polarity), and
    /// the melee-gated counterpart of `grants_self_attack_advantage`
    /// (which fires on either lane). Read by `compute_attack_mode` only
    /// when `is_melee`; a new "advantage, but only with a weapon in
    /// hand" rider lands as a one-line addition here rather than a
    /// fresh branch at the call site.
    pub fn grants_self_melee_attack_advantage(&self) -> bool {
        matches!(self, Condition::Shadowstepping)
    }

    /// True if the holder cannot take Reactions while this condition is
    /// up. Covers the explicit `NoReaction` lockout and the new `Confused`
    /// clause (RAW: chaos table prevents reactions). Read by
    /// `can_consume_resource` alongside the `blocks_action_economy` cohort
    /// so the gate has one chokepoint per resource lane.
    pub fn blocks_reactions(&self) -> bool {
        matches!(
            self,
            Condition::NoReaction
                | Condition::Confused
                | Condition::Sphered
                | Condition::Dominated
        )
    }

    /// True if the holder cannot consume any SpellSlot resource while
    /// this condition is up. The canonical case is `Silenced` (5e
    /// Silence spell): no verbal components means no leveled spells.
    /// Read by `can_consume_resource`'s SpellSlot gate alongside the
    /// action-blocked check. Cantrips (no slot cost) are unaffected.
    pub fn blocks_spell_slots(&self) -> bool {
        // `WildShaped` joins from the opposite direction to `Silenced`:
        // a hush imposed on you versus a body you chose. Same
        // mechanical place — RAW's Wild Shape reads "you can't cast
        // spells" — which is why the gate is a cohort rather than a
        // `Silenced` special-case. Combat Wild Shape's slot-to-HP
        // conversion is deliberately not blocked; RAW phrases it as
        // expending a slot rather than casting, and `WildHeal` drains
        // the slot manager directly rather than routing a `SpellSlot`
        // cost through here.
        matches!(self, Condition::Silenced | Condition::WildShaped)
    }

    /// True if the holder can't cast **any** spell while this condition
    /// is up — cantrips included. Read at the action layer by
    /// `Action::validate_input`, which identifies a spell as an action
    /// with a `school()` (the same marker `CastContext::is_cantrip`
    /// uses, kept complete for cantrips by
    /// `every_cantrip_declares_its_school`).
    ///
    /// Strictly stronger than `blocks_spell_slots`, and a superset of
    /// it: that gate lives on the `SpellSlot` resource lane, so it can
    /// only ever reach spells that cost a slot. Cantrips cost none, so
    /// nothing stopped a `Silenced` caster from Fire Bolting or a
    /// wild-shaped druid — a bear — from casting Poison Spray. Both
    /// conditions' RAW text is unqualified: Silence blocks verbal
    /// components, and every SRD cantrip in the engine has one; Wild
    /// Shape says "you can't cast spells" flat.
    ///
    /// Kept as a separate cohort rather than folded into
    /// `blocks_spell_slots` because the two fire at different layers
    /// and the resource lane still has to bounce a slot-costing spell
    /// on its own (a caster can be handed a `SpellSlot` cost from
    /// paths that don't route through `validate_input`). A future
    /// condition that blocks only *levelled* casting — an
    /// anti-magic-field variant that leaves cantrips alone — would sit
    /// on `blocks_spell_slots` alone, which is why the narrower gate
    /// stays meaningful.
    pub fn blocks_spellcasting(&self) -> bool {
        matches!(self, Condition::Silenced | Condition::WildShaped)
    }

    /// True if the holder auto-fails STR and DEX saving throws.
    /// 5e Paralyzed / Stunned / Petrified / Unconscious / Asleep all
    /// share this clause: a creature physically locked out of the
    /// reflexive / strength response auto-fails the saves that test
    /// those abilities. Centralized so new "physically locked"
    /// conditions can opt in with a one-line change rather than a
    /// re-edit of `EncounterInstance::auto_fail_save`.
    pub fn auto_fails_str_dex_saves(&self) -> bool {
        matches!(
            self,
            Condition::Paralyzed
                | Condition::Stunned
                | Condition::Petrified
                | Condition::Unconscious
                | Condition::Asleep
        )
    }
}

impl std::fmt::Display for Condition {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// The three shapes of the 5e Circle of Stars Druid's **Starry Form**,
/// in the order the AI considers them.
///
/// RAW picks one shape per transformation, and re-transforming replaces
/// whatever was already up rather than layering on top of it. Since the
/// three are ordinary conditions with ordinary timers, nothing about
/// the condition machinery enforces that on its own — a druid who took
/// Archer on round one and Dragon on round four would otherwise be
/// standing in two constellations at once with the bonus-action bolt
/// *and* the concentration floor.
///
/// So the install path (`prime_exclusive_self_condition`) walks this
/// slice and emits a `RemoveCondition` for every shape that is not the
/// one being assumed. Keeping the roster here rather than spelling out
/// "the other two" at each of the three action sites means a fourth
/// shape — RAW's level-10 Twinkling Constellations does not add one, but
/// a homebrew circle could — is a single row rather than three edits
/// that have to agree with each other.
pub const STARRY_FORMS: &[Condition] = &[
    Condition::StarryFormArcher,
    Condition::StarryFormChalice,
    Condition::StarryFormDragon,
];

/// The six **Arcane Shot** options of the 5e Arcane Archer Fighter,
/// in the order the AI considers them.
///
/// The second family to ride `ExclusivePrime`, and it needs the strip
/// for a sharper reason than Starry Form does. The archer's pool is two
/// charges deep, and RAW nocks one option onto one arrow: "you can use
/// only one Arcane Shot option per attack". Without the strip, nocking
/// Grasping on turn one and Shadow on turn two and then firing once
/// would cash both riders — a Restrained *and* Blinded target off a
/// single shot, which is not an option the subclass offers at any
/// level.
///
/// The order is the AI's preference and it is load-bearing; see
/// `ARCANE_SHOT_ORDER` in the AI module for the reasoning.
pub const ARCANE_SHOTS: &[Condition] = &[
    Condition::ArcaneShotBanishing,
    Condition::ArcaneShotBeguiling,
    Condition::ArcaneShotBursting,
    Condition::ArcaneShotEnfeebling,
    Condition::ArcaneShotGrasping,
    Condition::ArcaneShotShadow,
];

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

