use {
    crate::verifying::{
        model_builder::{
            Assignment, Failure, Model, ModelBuilder, ModelExtractionError, Report, Status,
            StatusExtractionError, Success,
        },
        problem::smtlib::Problem,
    },
    std::{
        fmt::{self, Display},
        io::Write as _,
        process::{Command, Output, Stdio},
    },
    thiserror::Error,
};

#[derive(Error, Debug)]
pub enum Cvc5Error {
    #[error("unable to spawn CVC5 as a child process")]
    Spawn(#[source] std::io::Error),
    #[error("unable to write to CVC5's stdin")]
    Write(#[source] std::io::Error),
    #[error("unable to wait for CVC5")]
    Wait(#[source] std::io::Error),
    #[error("unable to convert output")]
    ConvertOutput(#[source] std::string::FromUtf8Error),
}

#[derive(Debug, Clone)]
pub struct Cvc5Output {
    pub stdout: String,
}

impl TryFrom<Output> for Cvc5Output {
    type Error = Cvc5Error;

    fn try_from(value: Output) -> Result<Self, Self::Error> {
        Ok(Cvc5Output {
            stdout: String::from_utf8(value.stdout).map_err(Cvc5Error::ConvertOutput)?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Cvc5Report {
    pub problem: Problem,
    pub output: Cvc5Output,
    //pub elapsed_time: Duration,
}

fn assignments(output: &str) -> Vec<Assignment> {
    let mut assignments = Vec::new();

    let mut lines = output.lines();
    lines.next(); // Discard sat/unsat message
    lines.next(); // Discard the first '('
    for line in lines {
        assignments.push(Assignment::from_cvc_str(line));
    }
    assignments.pop(); // Discard the last ')'
    assignments
}

impl Report for Cvc5Report {
    fn status(&self) -> Result<Status, StatusExtractionError> {
        self.output.stdout.parse()
    }

    fn model(&self) -> Result<Option<Model>, ModelExtractionError> {
        match self.status() {
            Ok(status) => match status {
                Status::Success(success) => match success {
                    Success::Satisfiable => {
                        let assignments = assignments(&self.output.stdout);
                        Ok(Some(Model { assignments }))
                    }
                },
                Status::Failure(Failure::Unknown) => {
                    let assignments = assignments(&self.output.stdout);
                    Ok(Some(Model { assignments }))
                }
                Status::Failure(Failure::Unsatisfiable) => Ok(None),
            },
            Err(error) => Err(ModelExtractionError::FailedStatusExtraction(error)),
        }
    }

    fn evaluate(&self) -> (String, Status, Option<Model>) {
        let final_status;
        let mut message = String::new();
        let mut model = None;
        match self.status() {
            Ok(status) => match status {
                Status::Success(Success::Satisfiable) => {
                    final_status = status;
                    match self.model() {
                        Ok(result) => match result {
                            Some(m) => {
                                message.push_str(&status.to_string());
                                model = Some(m);
                            }
                            None => {
                                message.push_str("missing model");
                            }
                        },

                        Err(_) => todo!(),
                    }
                }

                Status::Failure(Failure::Unknown) => {
                    final_status = status;
                    match self.model() {
                        Ok(result) => match result {
                            Some(candidate) => {
                                // convert to defined-funs, then re-run sat check
                                // let func = Assignment::from_cvc_str(raw);
                                // problem.add_defined_function();
                                message.push_str(&status.to_string());
                                model = Some(candidate);
                            }
                            None => {
                                message.push_str("missing candidate model");
                            }
                        },
                        Err(_) => todo!(),
                    }
                }

                Status::Failure(Failure::Unsatisfiable) => {
                    final_status = status;
                    message.push_str(&status.to_string());
                }
            },

            Err(_) => todo!(),
        }
        (message, final_status, model)
    }
}

impl Display for Cvc5Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "--- {} ---", self.problem.name)?;
        writeln!(f)?;

        match self.status() {
            Ok(status) => writeln!(f, "status: {status}")?,
            Err(error) => writeln!(f, "error: {error}")?,
        }
        writeln!(f)?;

        writeln!(f, "model:")?;
        match self.model() {
            Ok(result) => match result {
                Some(model) => writeln!(f, "{model}"),
                None => writeln!(f, "none"),
            },
            Err(error) => writeln!(f, "error: {error}"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Cvc5 {
    pub time_limit: usize,
    //pub cores: usize,
}

impl ModelBuilder for Cvc5 {
    type Error = Cvc5Error;
    type Report = Cvc5Report;

    // fn cores(&self) -> usize {
    //     if self.cores == 0 { 1 } else { self.cores }
    // }

    fn build(&self, problem: Problem) -> Result<Self::Report, Self::Error> {
        //let start_time = Instant::now();

        let time_limit = self.time_limit.to_string();

        let arguments = Vec::from_iter(["--dump-models", "--tlimit", &time_limit]);

        let mut child = Command::new("cvc5")
            .args(arguments)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(Cvc5Error::Spawn)?;

        let mut stdin = child.stdin.take().unwrap();
        write!(stdin, "{problem}").map_err(Cvc5Error::Write)?;
        drop(stdin);

        let output = child
            .wait_with_output()
            .map_err(Cvc5Error::Wait)?
            .try_into()?;

        Ok(Cvc5Report {
            problem,
            output,
            //elapsed_time: start_time.elapsed(),
        })
    }
}
