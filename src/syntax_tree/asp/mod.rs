pub mod mini_gringo;

pub trait Definite {
    fn definite(&self) -> bool;
}

impl Definite for mini_gringo::AtomicFormula {
    fn definite(&self) -> bool {
        match self {
            mini_gringo::AtomicFormula::Literal(literal) => {
                matches!(literal.sign, mini_gringo::Sign::NoSign)
            }
            mini_gringo::AtomicFormula::Comparison(_) => true,
        }
    }
}

impl Definite for mini_gringo::Rule {
    fn definite(&self) -> bool {
        match self.head {
            mini_gringo::Head::Choice(_) => false,
            mini_gringo::Head::Basic(_) | mini_gringo::Head::Falsity => {
                let mut flag = true;
                for formula in self.body.formulas.iter() {
                    if !formula.definite() {
                        flag = false;
                    }
                }
                flag
            }
        }
    }
}

impl Definite for mini_gringo::Program {
    fn definite(&self) -> bool {
        for rule in self.rules.iter() {
            if !rule.definite() {
                return false;
            }
        }
        true
    }
}
