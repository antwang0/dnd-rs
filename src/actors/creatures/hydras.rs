use crate::actions::class_features::{SWIM_SPEED_TAG, UNDERWATER_BREATHING_TAG};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{HYDRA_BITE, HYDRA_MULTI};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, Skill, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Hydra — CR 8 monstrosity. Huge multi-headed serpent whose whole fight
/// is its head count.
///
/// SRD 5.2 **Multiple Heads**, in full: *"The hydra has five heads.
/// Whenever the hydra takes 25 damage or more on a single turn, one of
/// its heads dies. The hydra dies if all its heads are dead. At the end
/// of each of its turns when it has at least one living head, the hydra
/// grows two heads for each of its heads that died since its last turn,
/// unless it has taken Fire damage since its last turn. The hydra
/// regains 20 Hit Points when it grows new heads."* It is on the
/// template as `heads: 5` and resolved at
/// `EncounterInstance::resolve_severed_heads`.
///
/// That paragraph is a tactic, and the thing it replaced was not. The
/// hydra used to carry `regen_per_round: 10` with a docstring saying so:
/// *"the MM hydra's signature feature — 'as long as a head remains
/// alive, severed heads regrow' — is approximated as a flat regen (we
/// don't model head-counting / fire-cauterize mechanics)."* Ten hit
/// points a round is a longer health bar. The real clause asks a party
/// three questions: can you land twenty-five points inside one turn, do
/// you have fire, and what happens if you do the first without the
/// second — because a hydra that is chipped at rather than burned comes
/// back with *more* heads than it lost, and bites once more per turn for
/// each of them.
///
/// The regeneration is gone with it. RAW gives the hydra no per-round
/// heal at all; the twenty hit points it gets back are the regrowth's,
/// and they arrive only on the turns it grows something.
///
/// Stat profile (SRD 5.2): AC 15, ~184 HP (16d12+80), STR 20, DEX 12,
/// CON 20. Five heads and a bite apiece. No language slot (the hydra is
/// non-sentient).
///
/// **Reactive Heads** — *"for each head the hydra has beyond one, it
/// gets an extra Reaction that can be used only for Opportunity
/// Attacks"* — rides `ActorInstance::has_opportunity_reaction`, and the
/// "only" in that sentence is why it is a second predicate rather than a
/// bigger `reaction_slots`: every other reaction lane in the engine
/// bills from the one slot, and four extra slots in that pool would be a
/// hydra that could Counterspell five times.
///
/// It is also the half that makes the heads matter on the *party's*
/// turn rather than the hydra's. A five-headed hydra punishes five
/// people for stepping out of its reach, and a party that has taken
/// three heads off has bought itself three free withdrawals.
pub static HYDRA_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*HYDRA_MULTI);
    actions.push(&HYDRA_BITE);
    CreatureTemplate {
        name: "Hydra",
        // 'Y' (uppercase) — distinct from 'y' (Wyvern), 'H' (Hippogriff),
        // 'h' (Hell Hound). Visual reads as a tall serpent-headed beast.
        glyph: 'Y',
        ac: 15,
        // RAW speed line: Speed 40 ft., Swim 40 ft. — the same number
        // either way, so the swim tag below is the whole of the water
        // half.
        speed: 40.,
        // 16d12+80 ≈ 184 average per MM (CR 8).
        hitpoints: "16d12+80".parse().unwrap(),
        strength: 20,
        dexterity: 12,
        constitution: 20,
        intelligence: 2,
        wisdom: 10,
        charisma: 7,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        cr: 8.0,
        size: Size::Huge,
        creature_type: CreatureType::Monstrosity,
        actions,
        // Hydras are mindless; no Charm / Frighten resistance — they
        // simply don't process those effects (we leave the immunity
        // off to keep the spell list interactive).
        // 5e **Multiple Heads**: "advantage on saving throws against
        // being blinded, charmed, deafened, frightened, stunned, and
        // knocked unconscious." The hydra had been carrying a bare
        // `Unconscious` immunity in its place — one of the six, rounded
        // the wrong way. See `CreatureTemplate::has_multiple_heads` for
        // the other five and for why they are advantage rather than the
        // immunity SRD 5.2 prints.
        has_multiple_heads: true,
        // SRD 5.2's five heads. What used to be `regen_per_round: 10`
        // standing in for them — see the docstring above.
        heads: 5,
        has_extra_attack: true,
        // RAW swim speed: the tag is what makes `TerrainType::Water`
        // free to cross and lifts the underwater melee penalty.
        features: HashSet::from([SWIM_SPEED_TAG, UNDERWATER_BREATHING_TAG]),
        skills: HashSet::from([Skill::Perception]),
        ..CreatureTemplate::defaults()
    }
});
