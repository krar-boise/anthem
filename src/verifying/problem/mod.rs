use {
    anyhow::{Context as _, Result},
    std::{fmt, fs::File, io::Write as _, path::Path},
};

pub mod smtlib;
pub mod tptp;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Interpretation {
    Standard,
    Integer,
}

impl fmt::Display for Interpretation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Interpretation::Standard => write!(f, include_str!("standard_interpretation.p")),
            Interpretation::Integer => Ok(()),
        }
    }
}

impl Interpretation {
    pub fn to_file<P: AsRef<Path>>(self, path: P) -> Result<()> {
        let path = path.as_ref();
        let mut file = File::create(path)
            .with_context(|| format!("could not create file `{}`", path.display()))?;
        write!(file, "{self}").with_context(|| format!("could not write file `{}`", path.display()))
    }
}
