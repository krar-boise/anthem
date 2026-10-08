use {
    crate::verifying::{
        model_builder::{
            Assignment, Model, ModelBuilder, ModelExtractionError, Report, Status,
            StatusExtractionError, Success,
        },
        problem::smtlib::Problem,
    },
    std::{
        fmt::{self, Display},
        io::Write as _,
        process::{Command, Output, Stdio},
        //time::{Duration, Instant},
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

impl Report for Cvc5Report {
    fn status(&self) -> Result<Status, StatusExtractionError> {
        self.output.stdout.parse()
    }

    fn model(&self) -> Result<Option<Model>, ModelExtractionError> {
        match self.status() {
            Ok(status) => match status {
                Status::Success(success) => match success {
                    Success::Satisfiable => {
                        let mut assignments = Vec::new();

                        let output = &self.output.stdout;
                        let mut lines = output.lines();
                        lines.next(); // Discard sat/unsat message
                        for line in lines {
                            assignments.push(Assignment {
                                raw: line.to_string(),
                            });
                        }

                        Ok(Some(Model { assignments }))
                    }
                },
                Status::Failure(_) => Ok(None),
            },
            Err(error) => Err(ModelExtractionError::FailedStatusExtraction(error)),
        }
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

        let arguments = Vec::from_iter(["--tlimit", &time_limit]);

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
