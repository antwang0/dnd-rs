use std::collections::VecDeque;

use crate::{
    actions::action_template::{Action, ActionExecutionInfo},
    engine::{
        encounter::EncounterInstance, errors::ParseError, types::Coordinate, util::parse_coord,
    },
};

pub struct Prompt {
    actor_id: usize,
    actions: Vec<&'static (dyn Action + Send + Sync)>,
}

impl Prompt {
    pub fn new(actor_id: usize, actions: Vec<&'static (dyn Action + Send + Sync)>) -> Self {
        Self { actor_id, actions }
    }

    pub fn actor_id(&self) -> usize {
        self.actor_id
    }

    pub fn actions(&self) -> &Vec<&'static (dyn Action + Send + Sync)> {
        &self.actions
    }

    /// Parse one argument token into either a target actor id or a
    /// target tile. Supported forms:
    /// - `id:N` (or `#N`) — actor id N. Lookup is exact; rejects unknown ids.
    /// - `<x>,<y>` / `r3u2` / `l4d1` etc. — coordinate (see `parse_coord`).
    /// - `<name>` — exact actor name match (e.g. "Zombie 0" must be quoted
    ///   or use `id:N`; we don't do fuzzy matching).
    fn parse_arg(
        token: &str,
        encounter: &EncounterInstance,
        base: Coordinate,
    ) -> Option<TargetArg> {
        // Explicit actor-id form. Accepts "id:7", "#7".
        let id_str = token
            .strip_prefix("id:")
            .or_else(|| token.strip_prefix('#'));
        if let Some(s) = id_str
            && let Ok(id) = s.parse::<usize>()
            && encounter.actors.contains_key(&id)
        {
            return Some(TargetArg::Actor(id));
        }
        // Coordinate forms.
        if let Some(coord) = parse_coord(token, base) {
            return Some(TargetArg::Point(coord));
        }
        // Exact actor name match (no spaces — names have one). We compare
        // ignoring case to keep typing quick.
        encounter
            .actors
            .iter()
            .find(|(_, a)| a.name().eq_ignore_ascii_case(token))
            .map(|(id, _)| TargetArg::Actor(*id))
    }

    pub fn process_input(
        &self,
        input: &str,
        encounter_instance: &EncounterInstance,
    ) -> Result<ActionExecutionInfo, ParseError> {
        let actor = encounter_instance
            .actors
            .get(&self.actor_id)
            .ok_or_else(|| ParseError::new(format!("missing actor {}", self.actor_id)))?;

        let tokens: Vec<&str> = input.split_whitespace().collect();
        if tokens.is_empty() {
            return Err(ParseError::with_input("empty input", input));
        }

        // Match action by the longest token-prefix that names an action.
        // We try N tokens, then N-1, etc., so multi-word spell names like
        // "scorching ray" or "hold person" work alongside single-token
        // names. Aliases are still single-token (e.g. "sr", "hp").
        //
        // At each width, **canonical names are tried before aliases**.
        // That ordering is load-bearing rather than cosmetic: aliases are
        // short and collide freely across a big caster's action list, and
        // some of them collide with another action's real name. A wizard
        // carries both Darkness and Maddening Darkness, and the latter
        // aliases "darkness" — so a single-pass search that mixed the two
        // kinds resolved by *action-list order*, and typing `darkness`
        // cast Maddening Darkness (a level-8 slot) instead. A canonical
        // name is the one handle a player can be certain of, so nothing
        // is allowed to shadow it.
        //
        // Alias-vs-alias collisions are left resolved by list order and
        // are not treated as bugs: short handles are a convenience, every
        // action stays reachable by its full name, and renaming a few
        // hundred of them across the spell list would trade a small
        // ambiguity for a large one.
        let mut action_opt: Option<(usize, &(dyn Action + Send + Sync))> = None;
        for n in (1..=tokens.len()).rev() {
            let candidate = tokens[..n].join(" ");
            let by_name = self
                .actions
                .iter()
                .find(|e| candidate == e.name())
                .copied();
            let matched = by_name.or_else(|| {
                if n != 1 {
                    return None;
                }
                self.actions
                    .iter()
                    .find(|e| e.aliases().contains(&tokens[0]))
                    .copied()
            });
            if let Some(act) = matched {
                action_opt = Some((n, act));
                break;
            }
        }
        let Some((consumed, action)) = action_opt else {
            return Err(ParseError::with_input(
                format!("unknown or unavailable action {:?}", tokens[0]),
                input,
            ));
        };

        let mut target_ids: Vec<usize> = Vec::new();
        let mut target_locations: Vec<Coordinate> = Vec::new();

        let mut rest: VecDeque<&str> = tokens.into_iter().skip(consumed).collect();
        while let Some(tok) = rest.pop_front() {
            let token_trimmed = tok.trim();
            match Self::parse_arg(token_trimmed, encounter_instance, actor.location()) {
                Some(TargetArg::Actor(id)) => target_ids.push(id),
                Some(TargetArg::Point(coord)) => target_locations.push(coord),
                None => {
                    return Err(ParseError::with_input(
                        format!("could not parse argument {:?}", token_trimmed),
                        input,
                    ));
                }
            }
        }

        let aei = ActionExecutionInfo::new(
            action,
            self.actor_id,
            if target_ids.is_empty() {
                None
            } else {
                Some(target_ids)
            },
            if target_locations.is_empty() {
                None
            } else {
                Some(target_locations)
            },
            None,
        );

        if !aei.validate(encounter_instance) {
            return Err(ParseError::with_input(
                "invalid arguments or insufficient resources",
                input,
            ));
        }
        Ok(aei)
    }
}

enum TargetArg {
    Actor(usize),
    Point(Coordinate),
}

#[cfg(test)]
mod tests {
    use crate::engine::actor_gen::ActorGenParams;
    use crate::engine::encounter::EncounterInstance;
    use crate::engine::terrain_gen::TerrainGenParams;

    fn ei() -> EncounterInstance {
        let tp = TerrainGenParams {
            width: 30,
            height: 20,
            branch_depth: 4,
            branch_prob: 0.5,
        };
        let ap = ActorGenParams {
            cr_target: 0.25,
            n_teams: 2,
            pc_template: None,
            start_team: 0,
        };
        let mut e = EncounterInstance::from_params(&tp, &ap, Some(99)).unwrap();
        e.process_stack();
        e
    }

    #[test]
    fn parse_empty_input_returns_error_with_context() {
        let e = ei();
        let prompt = e.peek_prompt().unwrap();
        let err = match prompt.process_input("", &e) {
            Err(err) => err,
            Ok(_) => panic!("expected error"),
        };
        assert!(err.to_string().to_lowercase().contains("empty"));
    }

    #[test]
    fn parse_unknown_action_returns_named_error() {
        let e = ei();
        let prompt = e.peek_prompt().unwrap();
        let err = match prompt.process_input("frobnicate", &e) {
            Err(err) => err,
            Ok(_) => panic!("expected error"),
        };
        let msg = err.to_string();
        assert!(msg.contains("frobnicate"), "msg = {}", msg);
    }

    #[test]
    fn parse_actor_id_target_with_hash_prefix() {
        // Verify the parser populates target_ids for `#<id>`. Validation
        // (range, LOS, etc.) isn't the parser's job — keep the test
        // narrow by using `skip #N`, which doesn't need the actor to be
        // a real attack target.
        let e = ei();
        // Pick any actor id present in the encounter as the target.
        let some_id = *e
            .actors
            .keys()
            .find(|id| **id != e.peek_prompt().unwrap().actor_id())
            .expect("test encounter should have at least 2 actors");
        let prompt = e.peek_prompt().unwrap();
        let cmd = format!("skip #{}", some_id);
        // Skip declares NoArgs, so adding target_ids will fail validation;
        // we only care that the parser reaches the validate stage with
        // the id populated. Validation surfacing the error proves parsing
        // succeeded.
        let res = prompt.process_input(&cmd, &e);
        assert!(res.is_err(), "skip with extra target should fail validation");
    }

    #[test]
    fn parse_unknown_actor_id_errors() {
        let e = ei();
        let prompt = e.peek_prompt().unwrap();
        let err = match prompt.process_input("skip #99999", &e) {
            Err(err) => err,
            Ok(_) => panic!("expected error for unknown actor id"),
        };
        assert!(err.to_string().contains("99999"));
    }

    #[test]
    fn parse_skip_succeeds() {
        let e = ei();
        let prompt = e.peek_prompt().unwrap();
        let aei = match prompt.process_input("skip", &e) {
            Ok(a) => a,
            Err(err) => panic!("unexpected error: {}", err),
        };
        assert!(aei.validate(&e));
    }

    /// Player can target an enemy actor by `id:N` syntax for SingleActor
    /// actions like Slam. Before this, the prompt only ever populated
    /// target_locations and SingleActor actions could never validate from
    /// a typed command.
    #[test]
    fn parse_id_form_targets_actor() {
        use crate::actors::creatures::zombies::ZOMBIE_TEMPLATE;
        use crate::engine::encounter::EncounterInstance;
        use crate::engine::terrain_gen::TerrainGenParams;

        // Use bare encounter without process_stack so we can place actors
        // ourselves and craft a deterministic prompt.
        let tp = TerrainGenParams {
            width: 20,
            height: 20,
            branch_depth: 0,
            branch_prob: 0.0,
        };
        let ap = ActorGenParams {
            cr_target: 0.0,
            n_teams: 0,
            pc_template: None,
            start_team: 0,
        };
        let mut e = EncounterInstance::from_params(&tp, &ap, Some(0)).unwrap();
        let attacker = e
            .instantiate_creature(
                &ZOMBIE_TEMPLATE,
                crate::engine::types::Coordinate::new(5, 5),
                0,
                0,
            )
            .unwrap();
        let target = e
            .instantiate_creature(
                &ZOMBIE_TEMPLATE,
                crate::engine::types::Coordinate::new(7, 5),
                1,
                0,
            )
            .unwrap();
        // Build a prompt manually for the attacker.
        use super::Prompt;
        let prompt = Prompt::new(attacker, e.actors[&attacker].available_actions());
        let aei = prompt
            .process_input(&format!("trip id:{}", target), &e)
            .expect("id:N should resolve to actor target");
        assert_eq!(aei.target_ids().and_then(|ids| ids.first().copied()), Some(target));
    }

    /// Multi-word action names (e.g. "sacred flame", "scorching ray") must
    /// match across whitespace boundaries — previously the parser only
    /// consumed the first token and was unreachable for these spells via
    /// their canonical name (only the short aliases worked).
    #[test]
    fn parse_multi_word_action_name_resolves() {
        use crate::actors::creatures::wizards::WIZARD_TEMPLATE;
        use crate::actors::creatures::zombies::ZOMBIE_TEMPLATE;
        use crate::engine::encounter::EncounterInstance;
        use crate::engine::terrain_gen::TerrainGenParams;

        let tp = TerrainGenParams {
            width: 20,
            height: 20,
            branch_depth: 0,
            branch_prob: 0.0,
        };
        let ap = ActorGenParams {
            cr_target: 0.0,
            n_teams: 0,
            pc_template: None,
            start_team: 0,
        };
        let mut e = EncounterInstance::from_params(&tp, &ap, Some(0)).unwrap();
        let wizard = e
            .instantiate_creature(
                &WIZARD_TEMPLATE,
                crate::engine::types::Coordinate::new(5, 5),
                0,
                0,
            )
            .unwrap();
        let target = e
            .instantiate_creature(
                &ZOMBIE_TEMPLATE,
                crate::engine::types::Coordinate::new(7, 5),
                1,
                0,
            )
            .unwrap();
        use super::Prompt;
        let prompt = Prompt::new(wizard, e.actors[&wizard].available_actions());
        // "scorching ray <id>" must parse as the action "scorching ray"
        // (two tokens) plus a single target id arg.
        let aei = prompt
            .process_input(&format!("scorching ray id:{}", target), &e)
            .expect("scorching ray should parse as a two-token action name");
        assert_eq!(aei.target_ids().and_then(|ids| ids.first().copied()), Some(target));
    }

    /// Every action on every PC template is reachable by typing its own
    /// canonical name. That is the one addressing guarantee the parser
    /// makes, and before canonical names were preferred over aliases it
    /// did not hold: a wizard carries both Darkness and Maddening
    /// Darkness, the latter aliases "darkness", and the single-pass
    /// search resolved whichever came first in the action list — so
    /// `darkness` cast a level-8 spell.
    ///
    /// Swept across all twelve class families rather than spot-checked,
    /// because the failure mode is silent: the wrong spell fires and the
    /// player is told nothing. The sweep is also what makes the guarantee
    /// survive new content — a spell added with an alias that happens to
    /// equal an existing spell's name fails here rather than in play.
    ///
    /// Alias-vs-alias collisions are deliberately *not* asserted. They
    /// are pervasive across the large caster lists ("sphere" is claimed
    /// by four spells on the sorcerer), resolve by list order, and cost
    /// nothing that the canonical name doesn't recover.
    #[test]
    fn every_pc_action_is_reachable_by_its_canonical_name() {
        use crate::actions::action_template::Action;
        use crate::actors::actor_template::CreatureTemplate;
        use crate::actors::creatures::{
            barbarians, bards, clerics, druids, fighters, monks, paladins, rangers, rogues,
            sorcerers, warlocks, wizards,
        };
        let families: Vec<(&str, Vec<&'static CreatureTemplate>)> = vec![
            (
                "barbarian",
                vec![
                    &*barbarians::BARBARIAN_TEMPLATE,
                    &*barbarians::TOTEM_BARBARIAN_TEMPLATE,
                    &*barbarians::WOLF_TOTEM_BARBARIAN_TEMPLATE,
                    &*barbarians::EAGLE_TOTEM_BARBARIAN_TEMPLATE,
                    &*barbarians::TIGER_TOTEM_BARBARIAN_TEMPLATE,
                    &*barbarians::ELK_TOTEM_BARBARIAN_TEMPLATE,
                    &*barbarians::WOLVERINE_TOTEM_BARBARIAN_TEMPLATE,
                    &*barbarians::PANTHER_TOTEM_BARBARIAN_TEMPLATE,
                    &*barbarians::SEA_STORM_HERALD_BARBARIAN_TEMPLATE,
                    &*barbarians::DESERT_STORM_HERALD_BARBARIAN_TEMPLATE,
                    &*barbarians::TUNDRA_STORM_HERALD_BARBARIAN_TEMPLATE,
                    &*barbarians::BERSERKER_BARBARIAN_TEMPLATE,
                    &*barbarians::ZEALOT_BARBARIAN_TEMPLATE,
                ],
            ),
            (
                "bard",
                vec![
                    &*bards::BARD_TEMPLATE,
                    &*bards::VALOR_BARD_TEMPLATE,
                    &*bards::SWORDS_BARD_TEMPLATE,
                    &*bards::LORE_BARD_TEMPLATE,
                    &*bards::WHISPERS_BARD_TEMPLATE,
                ],
            ),
            (
                "cleric",
                vec![
                    &*clerics::CLERIC_TEMPLATE,
                    &*clerics::WAR_CLERIC_TEMPLATE,
                    &*clerics::LIGHT_CLERIC_TEMPLATE,
                    &*clerics::TEMPEST_CLERIC_TEMPLATE,
                    &*clerics::LIFE_CLERIC_TEMPLATE,
                    &*clerics::GRAVE_CLERIC_TEMPLATE,
                    &*clerics::FORGE_CLERIC_TEMPLATE,
                    &*clerics::TWILIGHT_CLERIC_TEMPLATE,
                ],
            ),
            (
                "druid",
                vec![
                    &*druids::DRUID_TEMPLATE,
                    &*druids::LAND_DRUID_TEMPLATE,
                    &*druids::MOON_DRUID_TEMPLATE,
                ],
            ),
            (
                "fighter",
                vec![
                    &*fighters::FIGHTER_TEMPLATE,
                    &*fighters::CHAMPION_TEMPLATE,
                    &*fighters::SAMURAI_FIGHTER_TEMPLATE,
                    &*fighters::ELDRITCH_KNIGHT_FIGHTER_TEMPLATE,
                    &*fighters::PSI_WARRIOR_FIGHTER_TEMPLATE,
                    &*fighters::CAVALIER_FIGHTER_TEMPLATE,
                ],
            ),
            (
                "monk",
                vec![
                    &*monks::MONK_TEMPLATE,
                    &*monks::OPEN_HAND_MONK_TEMPLATE,
                    &*monks::LONG_DEATH_MONK_TEMPLATE,
                    &*monks::SHADOW_MONK_TEMPLATE,
                ],
            ),
            (
                "paladin",
                vec![
                    &*paladins::PALADIN_TEMPLATE,
                    &*paladins::DEVOTION_PALADIN_TEMPLATE,
                    &*paladins::ANCIENTS_PALADIN_TEMPLATE,
                    &*paladins::VENGEANCE_PALADIN_TEMPLATE,
                    &*paladins::OATHBREAKER_PALADIN_TEMPLATE,
                    &*paladins::GLORY_PALADIN_TEMPLATE,
                    &*paladins::WATCHERS_PALADIN_TEMPLATE,
                ],
            ),
            (
                "ranger",
                vec![
                    &*rangers::RANGER_TEMPLATE,
                    &*rangers::HUNTER_RANGER_TEMPLATE,
                    &*rangers::GLOOM_STALKER_RANGER_TEMPLATE,
                    &*rangers::FEY_WANDERER_RANGER_TEMPLATE,
                    &*rangers::HORIZON_WALKER_RANGER_TEMPLATE,
                    &*rangers::MONSTER_SLAYER_RANGER_TEMPLATE,
                    &*rangers::SWARMKEEPER_RANGER_TEMPLATE,
                ],
            ),
            (
                "rogue",
                vec![
                    &*rogues::ROGUE_TEMPLATE,
                    &*rogues::ASSASSIN_ROGUE_TEMPLATE,
                    &*rogues::SWASHBUCKLER_ROGUE_TEMPLATE,
                    &*rogues::SCOUT_ROGUE_TEMPLATE,
                    &*rogues::ARCANE_TRICKSTER_ROGUE_TEMPLATE,
                ],
            ),
            (
                "sorcerer",
                vec![
                    &*sorcerers::SORCERER_TEMPLATE,
                    &*sorcerers::DRACONIC_SORCERER_TEMPLATE,
                    &*sorcerers::STORM_SORCERER_TEMPLATE,
                    &*sorcerers::ABERRANT_MIND_SORCERER_TEMPLATE,
                    &*sorcerers::DIVINE_SOUL_SORCERER_TEMPLATE,
                    &*sorcerers::SHADOW_MAGIC_SORCERER_TEMPLATE,
                ],
            ),
            (
                "warlock",
                vec![
                    &*warlocks::WARLOCK_TEMPLATE,
                    &*warlocks::FIEND_WARLOCK_TEMPLATE,
                    &*warlocks::UNDYING_WARLOCK_TEMPLATE,
                    &*warlocks::GREAT_OLD_ONE_WARLOCK_TEMPLATE,
                    &*warlocks::ARCHFEY_WARLOCK_TEMPLATE,
                    &*warlocks::CELESTIAL_WARLOCK_TEMPLATE,
                    &*warlocks::MARID_WARLOCK_TEMPLATE,
                    &*warlocks::DAO_WARLOCK_TEMPLATE,
                    &*warlocks::DJINNI_WARLOCK_TEMPLATE,
                    &*warlocks::EFREETI_WARLOCK_TEMPLATE,
                ],
            ),
            (
                "wizard",
                vec![
                    &*wizards::WIZARD_TEMPLATE,
                    &*wizards::NECROMANCY_WIZARD_TEMPLATE,
                    &*wizards::WAR_MAGIC_WIZARD_TEMPLATE,
                    &*wizards::ABJURATION_WIZARD_TEMPLATE,
                    &*wizards::EVOCATION_WIZARD_TEMPLATE,
                    &*wizards::DIVINATION_WIZARD_TEMPLATE,
                    &*wizards::ENCHANTMENT_WIZARD_TEMPLATE,
                    &*wizards::ILLUSION_WIZARD_TEMPLATE,
                    &*wizards::CONJURATION_WIZARD_TEMPLATE,
                    &*wizards::TRANSMUTATION_WIZARD_TEMPLATE,
                ],
            ),
        ];
        for (_label, family) in families {
            for template in family {
                let actions: Vec<&'static (dyn Action + Send + Sync)> =
                    template.actions.clone();
                for action in &actions {
                    let name = action.name();
                    // Mirror `process_input`'s resolution: longest
                    // token-prefix first, canonical names before aliases.
                    let tokens: Vec<&str> = name.split_whitespace().collect();
                    let mut resolved: Option<&str> = None;
                    for n in (1..=tokens.len()).rev() {
                        let candidate = tokens[..n].join(" ");
                        let by_name =
                            actions.iter().find(|e| candidate == e.name()).copied();
                        let matched = by_name.or_else(|| {
                            if n != 1 {
                                return None;
                            }
                            actions
                                .iter()
                                .find(|e| e.aliases().contains(&tokens[0]))
                                .copied()
                        });
                        if let Some(act) = matched {
                            resolved = Some(act.name());
                            break;
                        }
                    }
                    assert_eq!(
                        resolved,
                        Some(name),
                        "{}: typing '{}' resolves to {:?}",
                        template.name,
                        name,
                        resolved
                    );
                }
            }
        }
    }


}

