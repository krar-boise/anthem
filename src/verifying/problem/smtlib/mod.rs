use {
    super::Interpretation,
    crate::{
        formatting::fol::sigma_0::smtlib,
        syntax_tree::fol::sigma_0::{Formula, FunctionConstant, Predicate, Theory},
        verifying::model_builder::Assignment,
    },
    anyhow::{Context as _, Result},
    indexmap::IndexSet,
    lazy_static::lazy_static,
    regex::Regex,
    std::{fmt, fs::File, io::Write as _, path::Path},
};

lazy_static! {
    static ref FN: Regex = Regex::new(
        r"\(define-fun (?<symbol>[[:word:]]+) \((?<args>.*))\) (?<val>[Int|Bool]) (?<def>.*)\)"
    )
    .unwrap();
}

//
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum Logic {
    Ufnia,
    Qfnia,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum Role {
    Assertion,
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Role::Assertion => write!(f, "assert"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct AnnotatedFormula {
    pub name: String,
    pub role: Role,
    pub formula: Formula,
}

impl AnnotatedFormula {
    pub fn predicates(&self) -> IndexSet<Predicate> {
        self.formula.predicates()
    }

    // pub fn symbols(&self) -> IndexSet<String> {
    //     self.formula.symbols()
    // }

    pub fn function_constants(&self) -> IndexSet<FunctionConstant> {
        self.formula.function_constants()
    }

    // pub fn rename_conflicting_symbols(self, possible_conflicts: &IndexSet<Predicate>) -> Self {
    //     AnnotatedFormula {
    //         role: self.role,
    //         formula: self.formula.rename_conflicting_symbols(possible_conflicts),
    //         name: self.name,
    //     }
    // }

    fn quantifier_free(&self) -> bool {
        self.formula.quantifier_free()
    }
}

impl fmt::Display for AnnotatedFormula {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let role = &self.role;
        let formula = crate::formatting::fol::sigma_0::smtlib::Format(&self.formula);
        writeln!(f, "({role} {formula})")
        //writeln!(f, "({role} ({formula} :named {name}))")
    }
}

impl Assignment {
    pub fn from_cvc_str(raw: &str) -> Assignment {
        let caps = FN.captures(&raw).unwrap();
        let arg_string = caps["args"].to_string();
        let arguments = arg_string.split(" ").map(|s| s.to_string()).collect();

        Assignment {
            symbol: caps["symbol"].to_string(),
            arguments,
            value_sort: caps["val"].to_string(),
            definition: caps["def"].to_string(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Problem {
    pub name: String,
    pub logic: Logic,
    pub interpretation: Interpretation,
    pub defined_fns: Vec<Assignment>,
    pub formulas: Vec<AnnotatedFormula>,
}

impl Problem {
    pub fn with_name<S: Into<String>>(name: S) -> Problem {
        Problem {
            name: name.into(),
            interpretation: Interpretation::Integer,
            formulas: vec![],
            defined_fns: vec![],
            logic: Logic::Ufnia, // Default to most general logic
        }
    }

    // Change to a simpler logic, if possible
    // TODO: simpler logic in absence of placeholders?
    pub fn update_logic(mut self) -> Self {
        let mut quantifier_free = true;
        for f in self.formulas.iter() {
            if !f.quantifier_free() {
                quantifier_free = false;
                break;
            }
        }
        let logic = match quantifier_free {
            true => Logic::Qfnia,
            false => Logic::Ufnia,
        };
        self.logic = logic;
        self
    }

    pub fn add_annotated_formulas(
        mut self,
        annotated_formulas: impl IntoIterator<Item = AnnotatedFormula>,
    ) -> Self {
        for anf in annotated_formulas {
            if anf.name.is_empty() {
                self.formulas.push(AnnotatedFormula {
                    name: "unnamed_formula".to_string(),
                    role: anf.role,
                    formula: anf.formula,
                });
            } else if anf.name.starts_with('_') {
                self.formulas.push(AnnotatedFormula {
                    name: format!("f{}", anf.name),
                    role: anf.role,
                    formula: anf.formula,
                });
            } else {
                self.formulas.push(anf);
            }
        }
        self
    }

    pub fn add_theory<F>(mut self, theory: Theory, mut annotate: F) -> Self
    where
        F: FnMut(usize, Formula) -> AnnotatedFormula,
    {
        for (i, formula) in theory.formulas.into_iter().enumerate() {
            self.formulas.push(annotate(i, formula))
        }
        self
    }

    // pub fn rename_conflicting_symbols(mut self) -> Self {
    //     let propositional_predicates =
    //         IndexSet::from_iter(self.predicates().into_iter().filter(|p| p.arity == 0));

    //     let formulas = self
    //         .formulas
    //         .into_iter()
    //         .map(|f| f.rename_conflicting_symbols(&propositional_predicates))
    //         .collect();
    //     self.formulas = formulas;
    //     self
    // }

    pub fn predicates(&self) -> IndexSet<Predicate> {
        let mut result = IndexSet::new();
        for formula in &self.formulas {
            result.extend(formula.predicates())
        }
        result
    }

    // pub fn symbols(&self) -> IndexSet<String> {
    //     let mut result = IndexSet::new();
    //     for formula in &self.formulas {
    //         result.extend(formula.symbols())
    //     }
    //     result
    // }

    pub fn function_constants(&self) -> IndexSet<FunctionConstant> {
        let mut result = IndexSet::new();
        for formula in &self.formulas {
            result.extend(formula.function_constants())
        }
        result
    }

    pub fn to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();
        let mut file = File::create(path)
            .with_context(|| format!("could not create file `{}`", path.display()))?;
        write!(file, "{self}").with_context(|| format!("could not write file `{}`", path.display()))
    }

    pub fn add_defined_functions(mut self, assignments: Vec<Assignment>) -> Self {
        for assignment in assignments {
            self.defined_fns.push(assignment);
        }
        self
    }
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Set options
        writeln!(f, "(set-option :finite-model-find true)")?;
        writeln!(f, "(set-option :produce-models true)")?;

        // Set logic
        match self.logic {
            Logic::Ufnia => writeln!(f, "(set-logic UFNIA)")?,
            Logic::Qfnia => writeln!(f, "(set-logic QF_NIA)")?,
        }

        // Write defined functions (if any)
        for function in self.defined_fns.iter() {
            writeln!(f, "{function}")?;
        }

        let defined_fn_symbols: Vec<String> = self
            .defined_fns
            .iter()
            .map(|f| f.symbol.clone())
            //.filter_map(|f| f.symbol.clone())
            .collect();

        // Declare predicates
        for predicate in self.predicates().iter() {
            if !defined_fn_symbols.contains(&predicate.symbol) {
                writeln!(f, "(declare-fun {} Bool)", smtlib::Format(predicate))?;
            }
        }

        // Declare functions
        for function in self.function_constants().iter() {
            if !defined_fn_symbols.contains(&function.name) {
                writeln!(f, "(declare-fun {} () Int)", smtlib::Format(function))?;
            }
        }

        // Write assertions
        for formula in self.formulas.iter() {
            writeln!(f, "{formula}")?;
        }

        // Set filename
        writeln!(f, "(set-info :filename {})", self.name)?;

        // Check satisfiability and get model
        writeln!(f, "(check-sat)")?;
        // for p in self.predicates().iter() {
        //     let symbol = &p.symbol;
        //     let arity = p.arity;
        //     writeln!(f, "(get-value ({symbol}_{arity}))")?;
        // }
        // TODO: get-value for placeholders

        Ok(())
    }
}
