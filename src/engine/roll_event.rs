use crate::engine::{
    side_effects::ApplicableSideEffect,
    types::{AbilityScoreType, Skill},
};

pub enum RollType {
    SavingThrow,
    Attack,
    Damage,
}

pub enum CheckType {
    SkillCheck(Skill),
    AbilityCheck(AbilityScoreType),
}

pub enum Outcome {
    Noop,
    Roll(DieRoll),
    Chain(Box<Outcome>),
    SideEffects(Vec<Box<dyn ApplicableSideEffect>>),
}

pub struct DieRoll {
    pub actor_id: usize,
    pub threshold: u32,
    pub roll_type: RollType,
    pub check_type: Option<CheckType>,
    pub success_result: Box<Outcome>,
    pub failure_result: Box<Outcome>,
}
