use {
    crate::syntax_tree::{
        Node,
        fol::sigma_0::{
            Atom, AtomicFormula, BinaryConnective, BinaryOperator, Comparison, Formula,
            FunctionConstant, GeneralTerm, IntegerTerm, Predicate, Quantification, Quantifier,
            Relation, Sort, SymbolicTerm, UnaryConnective, UnaryOperator, Variable,
        },
    },
    std::fmt::{self, Display, Formatter},
};

pub struct Format<'a, N: Node>(pub &'a N);

impl Display for Format<'_, UnaryOperator> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self.0 {
            UnaryOperator::Negative => write!(f, "-"),
        }
    }
}

impl Display for Format<'_, BinaryOperator> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self.0 {
            BinaryOperator::Add => write!(f, "+"),
            BinaryOperator::Subtract => write!(f, "-"),
            BinaryOperator::Multiply => write!(f, "*"),
        }
    }
}

impl Display for Format<'_, IntegerTerm> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self.0 {
            IntegerTerm::Numeral(n) => {
                if *n < 0 {
                    let m = n.abs();
                    write!(f, "(- {m})")?;
                } else {
                    write!(f, "{n}")?;
                }

                Ok(())
            }
            IntegerTerm::Variable(v) => write!(f, "{v}_i"),
            IntegerTerm::FunctionConstant(c) => write!(f, "{c}_i"),
            IntegerTerm::UnaryOperation { op, arg } => {
                let op = Format(op);
                let arg = Format(arg.as_ref());
                write!(f, "({op} {arg})")
            }
            IntegerTerm::BinaryOperation { op, lhs, rhs } => {
                let op = Format(op);
                let lhs = Format(lhs.as_ref());
                let rhs = Format(rhs.as_ref());
                write!(f, "({op} {lhs} {rhs})")
            }
        }
    }
}

impl Display for Format<'_, SymbolicTerm> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self.0 {
            SymbolicTerm::Symbol(s) => write!(f, "{s}"),
            SymbolicTerm::FunctionConstant(c) => write!(f, "{c}_s"),
            SymbolicTerm::Variable(v) => write!(f, "{v}_s"),
        }
    }
}

impl Display for Format<'_, GeneralTerm> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self.0 {
            GeneralTerm::Infimum => write!(f, "c__infimum__"),
            GeneralTerm::Supremum => write!(f, "c__supremum__"),
            GeneralTerm::FunctionConstant(c) => write!(f, "{c}_g"),
            GeneralTerm::Variable(v) => write!(f, "{v}_g"),
            GeneralTerm::IntegerTerm(t) => write!(f, "{}", Format(t)),
            GeneralTerm::SymbolicTerm(t) => write!(f, "f__symbolic__({})", Format(t)),
        }
    }
}

impl Display for Format<'_, Predicate> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let symbol = &self.0.symbol;
        let arity = self.0.arity;
        write!(f, "{symbol}_{arity} (")?;
        for _i in 0..arity {
            write!(f, " Int")?;
        }
        write!(f, " )")?;

        Ok(())
    }
}

impl Display for Format<'_, Atom> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let predicate = &self.0.predicate_symbol;
        let terms = &self.0.terms;
        let sorts = &self.0.argument_sorts;
        let arity = terms.len();

        if !terms.is_empty() {
            write!(f, "(")?;
        }
        write!(f, "{predicate}_{arity}")?;

        if !terms.is_empty() {
            let iter = terms.iter().zip(sorts).map(|(term, sort)| {
                match (term, sort) {
                    // Apply integer-only formatter
                    (GeneralTerm::IntegerTerm(i), Sort::Integer) => format!("{}", Format(i)),
                    (GeneralTerm::SymbolicTerm(s), Sort::Symbol) => format!("{}", Format(s)),

                    // Panic - integer-only conversion must have failed due to a bug
                    _ => {
                        panic!("term/sort mismatch in TPTP formatter")
                    }
                }
            });

            for term in iter {
                write!(f, " {term}")?;
            }
            write!(f, ")")?;
        }

        Ok(())
    }
}

impl Display for Format<'_, Relation> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self.0 {
            Relation::Equal => write!(f, "="),
            Relation::NotEqual => write!(f, "distinct"),
            Relation::Greater => write!(f, ">"),
            Relation::Less => write!(f, "<"),
            Relation::GreaterEqual => write!(f, ">="),
            Relation::LessEqual => write!(f, "<="),
        }
    }
}

impl Display for Format<'_, Comparison> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mut comparisons = Vec::from_iter(self.0.individuals());
        if comparisons.len() > 1 {
            write!(f, "(and")?;
            for (lhs, relation, rhs) in comparisons {
                write!(f, " ({} {} {})", Format(relation), Format(lhs), Format(rhs))?;
            }
            write!(f, ")")?;
        } else {
            let (lhs, relation, rhs) = comparisons.pop().unwrap();
            write!(f, "({} {} {})", Format(relation), Format(lhs), Format(rhs))?;
        }

        Ok(())
    }
}

impl Display for Format<'_, AtomicFormula> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self.0 {
            AtomicFormula::Truth => write!(f, "true"),
            AtomicFormula::Falsity => write!(f, "false"),
            AtomicFormula::Atom(a) => Format(a).fmt(f),
            AtomicFormula::Comparison(c) => Format(c).fmt(f),
        }
    }
}

impl Display for Format<'_, Quantifier> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self.0 {
            Quantifier::Forall => write!(f, "forall"),
            Quantifier::Exists => write!(f, "exists"),
        }
    }
}

impl Display for Format<'_, FunctionConstant> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let name = &self.0.name;
        let sort = &self.0.sort;

        match sort {
            Sort::Integer => write!(f, "{name}_i"),
            _ => panic!("only integer-sorted placeholders are currently supported"),
        }
    }
}

impl Display for Format<'_, Variable> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let name = &self.0.name;
        let sort = &self.0.sort;

        match sort {
            Sort::General => write!(f, "{name}_g"),
            Sort::Integer => write!(f, "{name}_i"),
            Sort::Symbol => write!(f, "{name}_s"),
        }
    }
}

impl Display for Format<'_, Quantification> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let variables = &self.0.variables;

        write!(f, "{} (", Format(&self.0.quantifier))?;

        for var in variables {
            match var.sort {
                Sort::Integer => write!(f, " ({} Int)", Format(var)),
                _ => panic!("non-integer variables are not yet supported in SMTLIB"),
            }?;
        }

        write!(f, " )")?;

        Ok(())
    }
}

impl Display for Format<'_, UnaryConnective> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self.0 {
            UnaryConnective::Negation => write!(f, "not"),
        }
    }
}

impl Display for Format<'_, BinaryConnective> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self.0 {
            BinaryConnective::Implication => write!(f, "=>"),
            BinaryConnective::Conjunction => write!(f, "and"),
            BinaryConnective::Disjunction => write!(f, "or"),
            BinaryConnective::Equivalence | BinaryConnective::ReverseImplication => {
                unreachable!("unsupported by SMTLIB")
            }
        }
    }
}

impl Display for Format<'_, Formula> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self.0 {
            Formula::AtomicFormula(a) => write!(f, "{}", Format(a)),
            Formula::UnaryFormula {
                connective,
                formula,
            } => write!(f, "({} {})", Format(connective), Format(formula.as_ref())),
            Formula::BinaryFormula {
                connective,
                lhs,
                rhs,
            } => match connective {
                BinaryConnective::Conjunction
                | BinaryConnective::Disjunction
                | BinaryConnective::Implication => write!(
                    f,
                    "({} {} {})",
                    Format(connective),
                    Format(lhs.as_ref()),
                    Format(rhs.as_ref())
                ),
                BinaryConnective::ReverseImplication => {
                    write!(f, "(=> {} {})", Format(rhs.as_ref()), Format(lhs.as_ref()))
                }
                BinaryConnective::Equivalence => {
                    write!(
                        f,
                        "(and (=> {} {}) (=> {} {}))",
                        Format(lhs.as_ref()),
                        Format(rhs.as_ref()),
                        Format(rhs.as_ref()),
                        Format(lhs.as_ref())
                    )
                }
            },
            Formula::QuantifiedFormula {
                quantification,
                formula,
            } => {
                write!(
                    f,
                    "({} {})",
                    Format(quantification),
                    Format(formula.as_ref())
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::Format,
        crate::syntax_tree::fol::{
            IntegerConversion,
            sigma_0::{AtomicFormula, Formula, GeneralTerm},
        },
    };

    #[test]
    fn format_term() {
        for (src, target) in [
            ("1", "1"),
            ("-1", "(- 1)"),
            //("|1|", "(abs 1)"),
            ("1 + 2", "(+ 1 2)"),
            ("1 - 2", "(- 1 2)"),
            ("1 * 2", "(* 1 2)"),
            ("1 * (2+N$i)", "(* 1 (+ 2 N_i))"),
        ] {
            let f: GeneralTerm = src.parse().unwrap();
            let src = Format(&f).to_string();
            let target = target.to_string();
            assert_eq!(src, target, "\n{src} \n!=\n{target}")
        }
    }

    #[test]
    fn format_atomic_formula() {
        for (src, target) in [
            ("#true", "true"),
            ("#false", "false"),
            ("p", "p_0"),
            ("p(1,2,3)", "(p_3 1 2 3)"),
            ("1 < 2", "(< 1 2)"),
            ("1 <= 2", "(<= 1 2)"),
            ("1 >= 2", "(>= 1 2)"),
            ("1 > 2", "(> 1 2)"),
            ("1 = 2", "(= 1 2)"),
            ("1 != 2", "(distinct 1 2)"),
            ("1 = 2 < 3", "(and (= 1 2) (< 2 3))"),
        ] {
            let f: AtomicFormula = src.parse().unwrap();
            let g = f.convert_to_integer_domain().unwrap();
            let src = Format(&g).to_string();
            let target = target.to_string();
            assert_eq!(src, target, "\n{src} \n!=\n{target}")
        }
    }

    #[test]
    fn format_formula() {
        for (src, target) in [
            ("not p", "(not p_0)"),
            ("p and q", "(and p_0 q_0)"),
            ("p -> q", "(=> p_0 q_0)"),
            ("p or q", "(or p_0 q_0)"),
            ("forall X$i (X$i < 5)", "(forall ( (N_i Int) ) (< N_i 5))"),
            (
                "exists X$i Y$i (X$i < 5 and p(Y$i))",
                "(exists ( (N_i Int) (N1_i Int) ) (and (< N_i 5) (p_1 N1_i)))",
            ),
        ] {
            let f: Formula = src.parse().unwrap();
            let g = f.convert_to_integer_domain().unwrap();
            let src = Format(&g).to_string();
            let target = target.to_string();
            assert_eq!(src, target, "\n{src} \n!=\n{target}")
        }
    }
}
