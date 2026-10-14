use {
    crate::verifying::problem::smtlib::{self, Problem},
    lazy_static::lazy_static,
    regex::Regex,
    std::{
        fmt::{Debug, Display},
        str::FromStr,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    },
    thiserror::Error,
};

pub mod cvc5;

lazy_static! {
    static ref STATUS: Regex = Regex::new(r"(?<status>[[:word:]]+)").unwrap();
}

#[derive(Debug, Error)]
pub enum StatusExtractionError {
    #[error("the status of this model building problem is missing")]
    Missing,
    #[error("the status of this model building problem is not recognized: `{0}`")]
    Unknown(String),
}

#[derive(Debug, Error)]
pub enum ModelExtractionError {
    FailedStatusExtraction(#[from] StatusExtractionError),
}

impl Display for ModelExtractionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModelExtractionError::FailedStatusExtraction(error) => write!(f, "{error}"),
        }
    }
}

// #[derive(Debug, Error)]
// pub enum ReportEvaluationError {
//     FailedReportEvaluation(#[from] ModelExtractionError),
// }

// impl Display for ReportEvaluationError {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         todo!()
//     }
// }

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Success {
    Satisfiable,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Failure {
    Unsatisfiable,
    Timeout,
    Unknown,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Status {
    Success(Success),
    Failure(Failure),
}

impl Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Status::Success(Success::Satisfiable) => "Satisfiable",
                Status::Failure(Failure::Unsatisfiable) => "Unsatisfiable",
                Status::Failure(Failure::Unknown) => "Unknown",
                Status::Failure(Failure::Timeout) => "Timeout",
            }
        )
    }
}

impl FromStr for Status {
    type Err = StatusExtractionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut lines = s.lines();
        match lines.next() {
            Some(line) => {
                let status = line.trim();
                match status {
                    "sat" => Ok(Self::Success(Success::Satisfiable)),
                    "unsat" => Ok(Self::Failure(Failure::Unsatisfiable)),
                    "unknown" => Ok(Self::Failure(Failure::Unknown)),
                    "cvc5 interrupted by timeout." => Ok(Self::Failure(Failure::Timeout)),
                    x => Err(StatusExtractionError::Unknown(x.to_string())),
                }
            }
            None => Err(StatusExtractionError::Missing),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Assignment {
    pub symbol: String,
    pub arguments: Vec<String>,
    pub value_sort: String,
    pub definition: String,
}

impl Display for Assignment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "(define-fun {} (", self.symbol)?;
        for arg in self.arguments.iter() {
            write!(f, " {arg}")?
        }
        write!(f, " ) {} {})\n", self.value_sort, self.definition)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Model {
    pub assignments: Vec<Assignment>,
}

impl Display for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "--- countermodel ---")?;
        for assignment in self.assignments.iter() {
            write!(f, "{assignment}")?;
        }
        writeln!(f, "--- ------------ ---")?;
        Ok(())
    }
}

pub trait Report: Display + Debug + Clone {
    fn status(&self) -> Result<Status, StatusExtractionError>;

    fn model(&self) -> Result<Option<Model>, ModelExtractionError>;

    fn evaluate(&self) -> (String, Status, Option<Model>);
}

pub trait ModelBuilder: Debug + Clone + Send + 'static {
    type Report: Report + Send;
    type Error: Send;

    //fn cores(&self) -> usize;

    fn build(&self, problem: Problem) -> Result<Self::Report, Self::Error>;

    // fn build_all(
    //     &self,
    //     problems: impl IntoIterator<Item = Problem> + 'static,
    // ) -> Box<dyn Iterator<Item = Result<Self::Report, Self::Error>>> {
    //     todo!()
    // }
}

pub enum ModelBuildingBackend {
    Cvc5(cvc5::Cvc5),
}

impl ModelBuildingBackend {
    pub(crate) fn execute_problems(
        &self,
        problems: Vec<smtlib::Problem>,
        early_stop: Arc<AtomicBool>,
        use_early_stop: bool,
    ) -> (String, Option<Model>) {
        let mut status;
        let mut message = String::new();
        let mut model = None;
        match self {
            ModelBuildingBackend::Cvc5(cvc5) => {
                for problem in problems {
                    // Problem may have to be re-run after being extended with a candidate model
                    let remodel_problem = problem.clone();

                    // Stop early if early termination is enabled and the ATP thread has finished
                    if use_early_stop && early_stop.load(Ordering::Relaxed) {
                        message.push_str(
                            "\nCountermodel building terminated early by theorem proving thread",
                        );
                        break;
                    }

                    match cvc5.build(problem) {
                        Ok(report) => {
                            let candidate_model;
                            (message, status, candidate_model) = report.evaluate();
                            match status {
                                Status::Success(Success::Satisfiable) => {
                                    model = candidate_model;
                                    if use_early_stop {
                                        early_stop.store(true, Ordering::Relaxed); // CMB finished early
                                    }
                                }

                                Status::Failure(Failure::Unknown) => {
                                    match candidate_model {
                                        Some(candidate) => {
                                            let rem_problem = remodel_problem
                                                .add_defined_functions(
                                                    candidate.assignments.clone(),
                                                );
                                            match cvc5.build(rem_problem) {
                                                Ok(rem_report) => {
                                                    let (rem_message, rem_status, _) =
                                                        rem_report.evaluate();
                                                    if matches!(
                                                        rem_status,
                                                        Status::Success(Success::Satisfiable)
                                                    ) {
                                                        model = Some(candidate);
                                                        message.push_str(&rem_message);
                                                        if use_early_stop {
                                                            early_stop
                                                                .store(true, Ordering::Relaxed); // CMB finished early
                                                        }
                                                    } else {
                                                        message.push_str(&rem_message);
                                                    }
                                                }
                                                Err(_) => todo!(),
                                            }
                                        }
                                        None => {
                                            message.push_str("missing model");
                                        }
                                    }
                                }

                                Status::Failure(Failure::Unsatisfiable) => todo!(),

                                Status::Failure(Failure::Timeout) => todo!(),
                            }
                        }

                        Err(err) => {
                            message.push_str(&err.to_string());
                        }
                    }
                }
            }
        }
        (message, model)
    }
}
