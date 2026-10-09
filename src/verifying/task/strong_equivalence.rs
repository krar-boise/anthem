use {
    crate::{
        command_line::arguments::{Decomposition, FormulaRepresentation},
        convenience::{
            apply::Apply as _,
            compose::Compose as _,
            with_warnings::{Result, WithWarnings},
        },
        simplifying::fol::sigma_0::{classic::CLASSIC, ht::HT, intuitionistic::INTUITIONISTIC},
        syntax_tree::{
            asp::{Definite, mini_gringo as asp},
            fol::{
                IntegerConversion,
                sigma_0::{self as fol, Formula, Theory},
            },
        },
        translating::{
            classical_reduction::gamma::{Gamma as _, Here as _, There as _},
            formula_representation::{mu::Mu as _, tau_star::TauStar as _},
        },
        verifying::{
            problem::{
                Interpretation, smtlib,
                tptp::{self, Problem},
            },
            task::{CounterModelTask, ProofSearchTask, Task, TaskProblems},
        },
    },
    std::fmt::Display,
    thiserror::Error,
};

#[derive(Error, Debug)]
pub enum StrongEquivalenceTaskWarning {
    CountermodelWarning(#[from] StrongCounterModelTaskWarning),
}

impl Display for StrongEquivalenceTaskWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StrongEquivalenceTaskWarning::CountermodelWarning(warning) => {
                writeln!(f, "{warning}")
            }
        }
    }
}

#[derive(Error, Debug)]
pub enum StrongEquivalenceTaskError {
    FailedIntegerConversion(#[from] anyhow::Error),
    CountermodelError(#[from] StrongCounterModelTaskError),
}

impl Display for StrongEquivalenceTaskError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StrongEquivalenceTaskError::FailedIntegerConversion(error) => {
                writeln!(f, "conversion to integer-only failed: {error}")
            }
            StrongEquivalenceTaskError::CountermodelError(error) => {
                writeln!(
                    f,
                    "failed to construct countermodel task due to error: {error}"
                )
            }
        }
    }
}

pub struct StrongEquivalenceTask {
    pub left: asp::Program,
    pub right: asp::Program,
    pub decomposition: Decomposition,
    pub direction: fol::Direction,
    pub formula_representation: FormulaRepresentation,
    pub simplify: bool,
    pub break_equivalences: bool,
    pub int_only: bool,
}

impl StrongEquivalenceTask {
    fn transition_axioms(&self) -> fol::Theory {
        fn transition(p: asp::Predicate) -> fol::Formula {
            let p: fol::Predicate = p.into();

            let hp = p.clone().to_formula().here();
            let tp = p.to_formula().there();

            let variables = hp.free_variables();

            fol::Formula::BinaryFormula {
                connective: fol::BinaryConnective::Implication,
                lhs: hp.into(),
                rhs: tp.into(),
            }
            .quantify(fol::Quantifier::Forall, variables.into_iter().collect())
        }

        let mut predicates = self.left.predicates();
        predicates.extend(self.right.predicates());

        fol::Theory {
            formulas: predicates.into_iter().map(transition).collect(),
        }
    }
}

impl Task for StrongEquivalenceTask {
    type Error = StrongEquivalenceTaskError;
    type Warning = StrongEquivalenceTaskWarning;

    fn decompose(self) -> Result<TaskProblems, Self::Warning, Self::Error> {
        let mut warnings = vec![];

        let mut interpretation = Interpretation::Standard;
        if self.int_only {
            interpretation = Interpretation::Integer;
        }

        let mut transition_axioms = self.transition_axioms(); // These are the "forall X (hp(X) -> tp(X))" axioms.

        // Check if both programs are definite
        let definite = { self.left.definite() && self.right.definite() };

        let mut left = match self.formula_representation {
            FormulaRepresentation::Mu => self.left.mu(),
            FormulaRepresentation::TauStar => self.left.tau_star(),
        };

        let mut right = match self.formula_representation {
            FormulaRepresentation::Mu => self.right.mu(),
            FormulaRepresentation::TauStar => self.right.tau_star(),
        };

        if self.simplify {
            let mut portfolio = [INTUITIONISTIC, HT].concat().into_iter().compose();
            left = left
                .into_iter()
                .map(|f| f.apply_fixpoint(&mut portfolio))
                .collect();
            right = right
                .into_iter()
                .map(|f| f.apply_fixpoint(&mut portfolio))
                .collect();
        }

        // gamma can be bypassed if programs are definite
        if !definite {
            left = left.gamma();
            right = right.gamma();
        }

        if self.simplify {
            let mut portfolio = [INTUITIONISTIC, HT, CLASSIC].concat().into_iter().compose();
            left = left
                .into_iter()
                .map(|f| f.apply_fixpoint(&mut portfolio))
                .collect();
            right = right
                .into_iter()
                .map(|f| f.apply_fixpoint(&mut portfolio))
                .collect();
        }

        if self.break_equivalences {
            left = crate::breaking::fol::sigma_0::ht::break_equivalences_theory(left);
            right = crate::breaking::fol::sigma_0::ht::break_equivalences_theory(right);
        }

        if self.int_only {
            left = left.convert_to_integer_domain()?;
            right = right.convert_to_integer_domain()?;
            transition_axioms = transition_axioms.convert_to_integer_domain()?;
        }

        // CM Building is always done in the Integer domain (for now)
        let mut cm_left = left.clone();
        let mut cm_right = right.clone();
        let mut cm_transition = transition_axioms.clone();
        if !self.int_only {
            cm_left = cm_left.convert_to_integer_domain()?;
            cm_right = cm_right.convert_to_integer_domain()?;
            cm_transition = cm_transition.convert_to_integer_domain()?;
        }

        let countermodel_task = StrongCounterModelTask {
            left: cm_left,
            right: cm_right,
            transition_axioms: cm_transition,
            definite,
        }
        .decompose()?;

        warnings.extend(
            countermodel_task
                .warnings
                .into_iter()
                .map(StrongEquivalenceTaskWarning::from),
        );

        let proof_task = ValidatedStrongEquivalenceTask {
            left,
            right,
            transition_axioms,
            definite,
            decomposition: self.decomposition,
            direction: self.direction,
            interpretation,
        }
        .decompose()?
        .preface_warnings(warnings);

        let task = WithWarnings {
            data: TaskProblems {
                proof_problems: proof_task.data,
                countermodel_problems: countermodel_task.data,
            },
            warnings: proof_task.warnings,
        };

        Ok(task)
    }
}

struct ValidatedStrongEquivalenceTask {
    pub left: fol::Theory,
    pub right: fol::Theory,
    pub transition_axioms: fol::Theory,
    pub definite: bool,
    pub decomposition: Decomposition,
    pub direction: fol::Direction,
    pub interpretation: Interpretation,
}

impl ProofSearchTask for ValidatedStrongEquivalenceTask {
    type Error = StrongEquivalenceTaskError;
    type Warning = StrongEquivalenceTaskWarning;

    fn decompose(self) -> Result<Vec<Problem>, Self::Warning, Self::Error> {
        // Transition axioms are not needed for definite problems
        let transition_axioms = match self.definite {
            true => Theory::new(),
            false => self.transition_axioms,
        };

        let mut problems = Vec::new();
        if matches!(
            self.direction,
            fol::Direction::Universal | fol::Direction::Forward
        ) {
            problems.push(
                Problem::with_name("forward")
                    .set_interpretation(self.interpretation)
                    .add_theory(transition_axioms.clone(), |i, formula| {
                        tptp::AnnotatedFormula {
                            name: format!("transition_axiom_{i}"),
                            role: tptp::Role::Axiom,
                            formula,
                        }
                    })
                    .add_theory(self.left.clone(), |i, formula| tptp::AnnotatedFormula {
                        name: format!("left_{i}"),
                        role: tptp::Role::Axiom,
                        formula,
                    })
                    .add_theory(self.right.clone(), |i, formula| tptp::AnnotatedFormula {
                        name: format!("right_{i}"),
                        role: tptp::Role::Conjecture,
                        formula,
                    })
                    .rename_conflicting_symbols()
                    .create_unique_formula_names(),
            );
        }
        if matches!(
            self.direction,
            fol::Direction::Universal | fol::Direction::Backward
        ) {
            problems.push(
                Problem::with_name("backward")
                    .set_interpretation(self.interpretation)
                    .add_theory(transition_axioms, |i, formula| tptp::AnnotatedFormula {
                        name: format!("transition_axiom_{i}"),
                        role: tptp::Role::Axiom,
                        formula,
                    })
                    .add_theory(self.right, |i, formula| tptp::AnnotatedFormula {
                        name: format!("right_{i}"),
                        role: tptp::Role::Axiom,
                        formula,
                    })
                    .add_theory(self.left, |i, formula| tptp::AnnotatedFormula {
                        name: format!("left_{i}"),
                        role: tptp::Role::Conjecture,
                        formula,
                    })
                    .rename_conflicting_symbols()
                    .create_unique_formula_names(),
            );
        }

        let mut expanded_problems = vec![];
        for problem in problems {
            expanded_problems.append(&mut problem.decompose(self.decomposition));
        }

        Ok(WithWarnings::flawless(expanded_problems))
    }
}

pub struct StrongCounterModelTask {
    pub left: fol::Theory,
    pub right: fol::Theory,
    pub transition_axioms: fol::Theory,
    pub definite: bool,
}

#[derive(Error, Debug)]
pub enum StrongCounterModelTaskWarning {}

#[derive(Error, Debug)]
pub enum StrongCounterModelTaskError {}

impl CounterModelTask for StrongCounterModelTask {
    type Error = StrongCounterModelTaskError;

    type Warning = StrongCounterModelTaskWarning;

    fn decompose(self) -> Result<Vec<smtlib::Problem>, Self::Warning, Self::Error> {
        let transition_axioms = match self.definite {
            true => Theory { formulas: vec![] },
            false => self.transition_axioms,
        };

        // not (lhs <=> rhs)
        let lhs = Box::new(Formula::conjoin(self.left.formulas));
        let rhs = Box::new(Formula::conjoin(self.right.formulas));
        let consequent = Formula::UnaryFormula {
            connective: fol::UnaryConnective::Negation,
            formula: Formula::BinaryFormula {
                connective: fol::BinaryConnective::Equivalence,
                lhs,
                rhs,
            }
            .into(),
        };

        let problem = smtlib::Problem::with_name("countermodel")
            .add_theory(transition_axioms, |i, formula| smtlib::AnnotatedFormula {
                name: format!("transition_axiom_{i}"),
                role: smtlib::Role::Assertion,
                formula,
            })
            .add_annotated_formulas(vec![consequent].into_iter().map(|formula| {
                smtlib::AnnotatedFormula {
                    name: "consequent".to_string(),
                    role: smtlib::Role::Assertion,
                    formula,
                }
            }))
            .update_logic();

        Ok(WithWarnings::flawless(vec![problem]))
    }
}
