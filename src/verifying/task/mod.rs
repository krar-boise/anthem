pub mod external_equivalence;
pub mod strong_equivalence;

use crate::{convenience::with_warnings::Result, verifying::problem::tptp::Problem};

pub trait Task {
    type Error;
    type Warning;
    fn decompose(self) -> Result<Vec<Problem>, Self::Warning, Self::Error>;
}
