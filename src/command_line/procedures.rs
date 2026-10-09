use {
    crate::{
        analyzing::{regularity::Regularity as _, tightness::Tightness},
        command_line::{
            arguments::{
                self, Arguments, Command, Equivalence, Output, ParseAs, Property,
                SimplificationPortfolio, SimplificationStrategy, Translation,
            },
            files::Files,
        },
        convenience::{apply::Apply, compose::Compose},
        simplifying::fol::sigma_0::{classic::CLASSIC, ht::HT, intuitionistic::INTUITIONISTIC},
        syntax_tree::{Node as _, asp::mini_gringo as asp, fol::sigma_0 as fol},
        translating::{
            classical_reduction::{completion::Completion as _, gamma::Gamma as _},
            formula_representation::{mu::Mu as _, natural::Natural as _, tau_star::TauStar as _},
        },
        verifying::{
            model_builder::{ModelBuildingBackend, cvc5::Cvc5},
            problem::Interpretation,
            prover::{Prover, Report, Status, Success, vampire::Vampire},
            task::{
                Task, external_equivalence::ExternalEquivalenceTask,
                strong_equivalence::StrongEquivalenceTask,
            },
        },
    },
    anyhow::{Context, Result, anyhow},
    clap::Parser as _,
    either::Either,
    indexmap::IndexSet,
    std::{path::PathBuf, thread, time::Instant},
};

pub fn main() -> Result<()> {
    match Arguments::parse().command {
        Command::Analyze { property, input } => {
            match property {
                Property::Regularity => {
                    let program =
                        input.map_or_else(asp::Program::from_stdin, asp::Program::from_file)?;
                    let is_regular = program.is_regular();
                    println!("{is_regular}");
                }

                Property::Tightness => {
                    let program =
                        input.map_or_else(asp::Program::from_stdin, asp::Program::from_file)?;
                    let is_tight = program.is_tight();
                    println!("{is_tight}");
                }
            }

            Ok(())
        }

        Command::Parse {
            r#as,
            output,
            input,
        } => {
            match r#as {
                ParseAs::Program => {
                    let program =
                        input.map_or_else(asp::Program::from_stdin, asp::Program::from_file)?;
                    match output {
                        Output::Debug => println!("{program:#?}"),
                        Output::Default => print!("{program}"),
                    }
                }
                ParseAs::Theory => {
                    let theory =
                        input.map_or_else(fol::Theory::from_stdin, fol::Theory::from_file)?;
                    match output {
                        Output::Debug => println!("{theory:#?}"),
                        Output::Default => print!("{theory}"),
                    }
                }
                ParseAs::Specification => {
                    let specification = input.map_or_else(
                        fol::Specification::from_stdin,
                        fol::Specification::from_file,
                    )?;
                    match output {
                        Output::Debug => println!("{specification:#?}"),
                        Output::Default => print!("{specification}"),
                    }
                }
                ParseAs::UserGuide => {
                    let user_guide =
                        input.map_or_else(fol::UserGuide::from_stdin, fol::UserGuide::from_file)?;
                    match output {
                        Output::Debug => println!("{user_guide:#?}"),
                        Output::Default => print!("{user_guide}"),
                    }
                }
            };

            Ok(())
        }

        Command::Simplify {
            portfolio,
            strategy,
            input,
        } => {
            let mut simplification = match portfolio {
                SimplificationPortfolio::Classic => [INTUITIONISTIC, HT, CLASSIC].concat(),
                SimplificationPortfolio::Ht => [INTUITIONISTIC, HT].concat(),
                SimplificationPortfolio::Intuitionistic => [INTUITIONISTIC].concat(),
            }
            .into_iter()
            .compose();

            let theory = input.map_or_else(fol::Theory::from_stdin, fol::Theory::from_file)?;

            let simplified_theory: fol::Theory = theory
                .into_iter()
                .map(|formula| match strategy {
                    SimplificationStrategy::Shallow => simplification(formula),
                    SimplificationStrategy::Recursive => formula.apply(&mut simplification),
                    SimplificationStrategy::Fixpoint => formula.apply_fixpoint(&mut simplification),
                })
                .collect();

            print!("{simplified_theory}");

            Ok(())
        }

        Command::Translate { with, input } => {
            match with {
                Translation::Completion => {
                    let theory =
                        input.map_or_else(fol::Theory::from_stdin, fol::Theory::from_file)?;
                    let completed_theory = theory
                        .completion(IndexSet::new())
                        .context("the given theory is not completable")?;
                    print!("{completed_theory}")
                }

                Translation::Gamma => {
                    let theory =
                        input.map_or_else(fol::Theory::from_stdin, fol::Theory::from_file)?;
                    let gamma_theory = theory.gamma();
                    print!("{gamma_theory}")
                }

                Translation::Mu => {
                    let program =
                        input.map_or_else(asp::Program::from_stdin, asp::Program::from_file)?;
                    let theory = program.mu();
                    print!("{theory}")
                }

                Translation::Natural => {
                    let program =
                        input.map_or_else(asp::Program::from_stdin, asp::Program::from_file)?;
                    let theory = program
                        .natural()
                        .context("the given program is not regular")?;
                    print!("{theory}")
                }

                Translation::TauStar => {
                    let program =
                        input.map_or_else(asp::Program::from_stdin, asp::Program::from_file)?;
                    let theory = program.tau_star();
                    print!("{theory}")
                }
            }

            Ok(())
        }

        Command::Verify {
            equivalence,
            decomposition,
            direction,
            formula_representation,
            countermodel,
            bypass_tightness,
            no_simplify,
            no_eq_break,
            no_proof_search,
            no_timing,
            int_only,
            time_limit,
            prover_instances,
            prover_cores,
            save_problems: out_dir,
            files,
        } => {
            let start_time = Instant::now();

            let files =
                Files::sort(files).context("unable to sort the given files by their function")?;

            let with_countermodel = matches!(countermodel, arguments::ModelBuilder::Cvc5);

            let task_problems = match equivalence {

                Equivalence::Strong => {
                    let left = asp::Program::from_file(
                        files
                            .left()
                            .ok_or(anyhow!("no left program was provided"))?,
                    )?;
                    let right = asp::Program::from_file(
                        files
                            .right()
                            .ok_or(anyhow!("no right program was provided"))?,
                    )?;

                    match (countermodel, formula_representation) {
                        (arguments::ModelBuilder::Cvc5, arguments::FormulaRepresentation::TauStar) => {
                            return Err(anyhow!("tau-star formula representation is not yet supported for countermodel building"))
                        },
                        (arguments::ModelBuilder::None, ..)
                        | (arguments::ModelBuilder::Cvc5, arguments::FormulaRepresentation::Mu) => {
                            if !(left.is_regular() && right.is_regular()) {
                                return Err(anyhow!("only regular programs are currently supported for countermodel building"))
                            }
                        },
                    }

                    StrongEquivalenceTask {
                    left,
                    right,
                    decomposition,
                    formula_representation,
                    direction,
                    int_only,
                    simplify: !no_simplify,
                    break_equivalences: !no_eq_break,
                }
                .decompose()?
                .report_warnings()
            },

                Equivalence::External => ExternalEquivalenceTask {
                    specification: match files
                        .specification()
                        .ok_or(anyhow!("no specification was provided"))?
                    {
                        Either::Left(program) => Either::Left(asp::Program::from_file(program)?),
                        Either::Right(specification) => {
                            Either::Right(fol::Specification::from_file(specification)?)
                        }
                    },
                    program: asp::Program::from_file(
                        files.program().ok_or(anyhow!("no program was provided"))?,
                    )?,
                    user_guide: fol::UserGuide::from_file(
                        files
                            .user_guide()
                            .ok_or(anyhow!("no user guide was provided"))?,
                    )?,
                    proof_outline: files
                        .proof_outline()
                        .map(fol::Specification::from_file)
                        .unwrap_or_else(|| Ok(fol::Specification::empty()))?,
                    decomposition,
                    formula_representation,
                    direction,
                    bypass_tightness,
                    int_only,
                    simplify: !no_simplify,
                    break_equivalences: !no_eq_break,
                }
                .decompose()?
                .report_warnings(),
            };

            let problems = task_problems.proof_problems;

            if let Some(out_dir) = out_dir {
                let mut preamble_path = out_dir.clone();
                preamble_path.push("standard_preamble.p");
                // Write preamble to separate file
                Interpretation::Standard.to_file(&preamble_path)?;

                for problem in &problems {
                    let mut path = out_dir.clone();
                    path.push(format!("{}.p", problem.name));
                    let mut problem = problem.clone();
                    problem.preamble = Some(PathBuf::from("standard_preamble.p"));
                    problem.to_file(path)?;
                }

                if with_countermodel {
                    let countermodel_problems = task_problems.countermodel_problems.clone();
                    for problem in countermodel_problems {
                        let mut path = out_dir.clone();
                        path.push(format!("{}.smt2", problem.name));
                        let problem = problem.clone();
                        problem.to_file(path)?;
                    }
                }
            }

            if !no_proof_search {
                let prover = Vampire {
                    time_limit,
                    instances: prover_instances,
                    cores: prover_cores,
                };

                let problems = problems.into_iter().inspect(|problem| {
                    println!("> Proving {}...", problem.name);
                    println!("Axioms:");
                    for axiom in problem.axioms() {
                        println!("    {}", axiom.formula);
                    }
                    println!();
                    println!("Conjectures:");
                    for conjecture in problem.conjectures() {
                        println!("    {}", conjecture.formula);
                    }
                    println!();
                });

                // Run proof search and CM building in parallel
                let mut handle = None;
                if with_countermodel {
                    let backend = match countermodel {
                        arguments::ModelBuilder::Cvc5 => {
                            ModelBuildingBackend::Cvc5(Cvc5 { time_limit })
                        }
                        arguments::ModelBuilder::None => unreachable!(),
                    };

                    // TODO: an "unsat" status indicates the ATP problem is valid?
                    // Returns Some(model) if a countermodel is found
                    let thread_handle = thread::spawn(move || {
                        backend.execute_problems(task_problems.countermodel_problems)
                    });
                    handle = Some(thread_handle);
                }

                let mut prover_success = true;
                for result in prover.prove_all(problems) {
                    match result {
                        Ok(report) => match report.status() {
                            Ok(status) => {
                                println!(
                                    "> Proving {} ended with a SZS status",
                                    report.problem.name
                                );
                                print!("Status: {status}");
                                if !no_timing {
                                    print!(" ({} ms)", report.elapsed_time.as_millis())
                                }
                                println!();
                                if !matches!(status, Status::Success(Success::Theorem)) {
                                    prover_success = false;
                                }
                            }
                            Err(error) => {
                                println!(
                                    "> Proving {} ended without a SZS status",
                                    report.problem.name
                                );
                                println!("Output/stdout:");
                                println!("{}", report.output.stdout);
                                println!("Output/stderr:");
                                println!("{}", report.output.stderr);
                                println!("Error: {error}");
                                prover_success = false;
                            }
                        },
                        Err(error) => {
                            println!("> Proving <a problem> ended with an error"); // TODO: Get the name of the problem
                            println!("Error: {error}");
                            prover_success = false;
                        }
                    }
                    println!();
                }

                // Wait for CM building to finish
                let mut countermodel_found = false;
                if with_countermodel {
                    match handle.take().unwrap().join() {
                        Ok((message, model_result)) => match model_result {
                            Some(model) => {
                                countermodel_found = true;
                                println!("{model}");
                            }
                            None => println!("{message}"),
                        },
                        Err(err) => println!("{:?}", err),
                    }
                }

                if prover_success && !countermodel_found {
                    print!("> Success! Anthem found a proof of the theorem.")
                } else if !prover_success && countermodel_found {
                    print!("> Failure! Anthem found the preceding counterexample.");
                } else if prover_success && countermodel_found {
                    print!("> This is a bug! Anthem found a proof AND a counterexample.");
                } else {
                    print!(
                        "> Failure (Unknown)! Anthem was unable to find either a proof or disproof of the theorem."
                    )
                }

                if !no_timing {
                    print!(" ({} ms)", start_time.elapsed().as_millis())
                }

                println!()
            }

            Ok(())
        }
    }
}
