pub mod external_equivalence;
pub mod strong_equivalence;

use crate::{
    convenience::with_warnings::Result,
    verifying::problem::{smtlib, tptp},
};

pub struct TaskProblems {
    pub proof_problems: Vec<tptp::Problem>,
    pub countermodel_problems: Vec<smtlib::Problem>,
}

pub trait Task {
    type Error;
    type Warning;
    fn decompose(self) -> Result<TaskProblems, Self::Warning, Self::Error>;
}

pub trait ProofSearchTask {
    type Error;
    type Warning;
    fn decompose(self) -> Result<Vec<tptp::Problem>, Self::Warning, Self::Error>;
}

pub trait CounterModelTask {
    type Error;
    type Warning;
    fn decompose(self) -> Result<Vec<smtlib::Problem>, Self::Warning, Self::Error>;
}
