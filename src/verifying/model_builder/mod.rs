use {
    crate::verifying::problem::smtlib::{self, Problem},
    lazy_static::lazy_static,
    regex::Regex,
    std::{
        fmt::{Debug, Display},
        str::FromStr,
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

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Success {
    Satisfiable,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Failure {
    Unsatisfiable,
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
                    x => Err(StatusExtractionError::Unknown(x.to_string())),
                }
            }
            None => Err(StatusExtractionError::Missing),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Assignment {
    pub raw: String,
}

impl Display for Assignment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}", self.raw)
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
    ) -> (String, Option<Model>) {
        let mut message = String::new();
        let mut model = None;
        match self {
            ModelBuildingBackend::Cvc5(cvc5) => {
                for problem in problems {
                    match cvc5.build(problem) {
                        Ok(report) => match report.model() {
                            Ok(result) => match result {
                                Some(m) => {
                                    message = report.status().unwrap().to_string();
                                    model = Some(m);
                                }
                                None => {
                                    message = "missing model".to_string();
                                }
                            },
                            Err(err) => {
                                message = err.to_string();
                            }
                        },
                        Err(err) => {
                            message = err.to_string();
                        }
                    }
                }
            }
        }
        (message, model)
    }
}
