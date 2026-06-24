use std::time::{Duration, Instant, SystemTime};

use evilscheme::{Atom, EvalError, Evaluator};

fn repl() {
    let stdin = std::io::stdin();
    let mut scope = Evaluator::new_empty();

    let start_time = Instant::now();

    scope.bind_host_func(
        "runtime".into(),
        Box::new(move |args| {
            if args.is_empty() {
                Ok(Atom::Number(
                    Instant::now().duration_since(start_time).as_secs_f64(),
                ))
            } else {
                Err(EvalError::ArityMismatch {
                    expected: 0,
                    got: args.len(),
                })
            }
        }),
    );

    scope.bind_host_func(
        "dump".into(),
        Box::new(|args| {
            println!("{args:#?}");
            Ok(Atom::Nil)
        }),
    );

    scope.bind_host_func(
        "unix-time".into(),
        Box::new(|args| {
            if args.is_empty() {
                Ok(Atom::Number(
                    SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs_f64(),
                ))
            } else {
                Err(EvalError::ArityMismatch {
                    expected: 0,
                    got: args.len(),
                })
            }
        }),
    );

    loop {
        let mut src = String::new();
        stdin.read_line(&mut src).unwrap();

        if src.is_empty() {
            println!();
            break;
        }

        let atom = match Atom::parse(&src) {
            Ok(a) => a,
            Err(e) => {
                eprintln!(";; Parsing error: {}", e);
                continue;
            }
        };

        match scope.eval(&atom) {
            Ok(Atom::Nil) => {}
            Ok(a) => println!("; {}", a),
            Err(e) => eprintln!(";; Eval error: {}", e),
        }
    }
}

fn exec_file_once(file_path: &str) {
    let atom = Atom::parse(&std::fs::read_to_string(file_path).unwrap()).unwrap();
    let mut eval = Evaluator::new_empty();

    println!("{}", eval.eval(&atom).unwrap());
}

fn exec_file_loop(file_path: &str) {
    let atom = Atom::parse(&std::fs::read_to_string(file_path).unwrap()).unwrap();

    let mut scope = Evaluator::new_empty();
    let start_time = Instant::now();

    scope.bind_host_func(
        "runtime".into(),
        Box::new(move |args| {
            if args.is_empty() {
                Ok(Atom::Number(
                    Instant::now().duration_since(start_time).as_secs_f64(),
                ))
            } else {
                Err(EvalError::ArityMismatch {
                    expected: 0,
                    got: args.len(),
                })
            }
        }),
    );

    loop {
        eprint!("\x1b[2K{}\r", scope.eval(&atom).unwrap());
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        repl();
    } else {
        let file_path = args.last().expect(">= 1 cmdline argument");

        if args.contains(&String::from("--loop")) {
            exec_file_loop(file_path);
        } else {
            exec_file_once(file_path);
        }
    }
}
