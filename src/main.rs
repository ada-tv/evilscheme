use std::time::{Duration, Instant, SystemTime};

use evilscheme::{Atom, EvalError, Evaluator};

fn add_host_funcs(scope: &mut Evaluator) {
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
        "print".into(),
        Box::new(|args| {
            if args.is_empty() {
                println!();
                return Ok(Atom::Nil);
            }

            for arg in &args[1..] {
                println!("{arg} ");
            }

            println!("{}", args.last().expect("at least one item"));

            Ok(Atom::Nil)
        }),
    );

    scope.bind_host_func(
        "display".into(),
        Box::new(|args| {
            if args.len() != 1 {
                return Err(EvalError::ArityMismatch {
                    expected: 1,
                    got: args.len(),
                });
            }

            print!("{}", args[0]);

            Ok(Atom::Nil)
        }),
    );

    scope.bind_host_func(
        "newline".into(),
        Box::new(|args| {
            if args.len() != 0 {
                return Err(EvalError::ArityMismatch {
                    expected: 0,
                    got: args.len(),
                });
            }

            println!();

            Ok(Atom::Nil)
        }),
    );

    scope.bind_host_func("exit".into(), Box::new(|_| std::process::exit(0)));

    // why is scheme's epoch 1900
    const SCHEME_EPOCH: f64 = 2208988800.0;

    scope.set_in_current_scope("epoch".into(), Atom::Number(SCHEME_EPOCH));

    scope.bind_host_func(
        "get-universal-time".into(),
        Box::new(|args| {
            if args.is_empty() {
                Ok(Atom::Number(
                    (SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs_f64())
                        + SCHEME_EPOCH,
                ))
            } else {
                Err(EvalError::ArityMismatch {
                    expected: 0,
                    got: args.len(),
                })
            }
        }),
    );
}

fn repl() {
    println!("\x1b[92m;; Evil Scheme REPL\x1b[0m");

    let stdin = std::io::stdin();
    let mut scope = Evaluator::new_empty();
    add_host_funcs(&mut scope);

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
                eprintln!("\x1b[1;91m;; Parsing error: \x1b[0m{}", e);
                continue;
            }
        };

        match scope.eval(&atom) {
            Ok(Atom::Nil) => {}
            Ok(a) => println!("\x1b[92m;; {}\x1b[0m", a),
            Err(e) => eprintln!("\x1b[1;91m;; Eval error: \x1b[0m{}", e),
        }
    }
}

fn exec_file_once(file_path: &str) {
    let atom = match Atom::parse(&std::fs::read_to_string(file_path).unwrap()) {
        Ok(atom) => atom,
        Err(e) => {
            eprintln!("Parse error: {e}");
            return;
        }
    };

    let mut scope = Evaluator::new_empty();
    add_host_funcs(&mut scope);

    match scope.eval(&atom) {
        Ok(_) => {}
        Err(e) => eprintln!("Eval error: {e}"),
    }
}

fn exec_file_loop(file_path: &str) {
    let atom = match Atom::parse(&std::fs::read_to_string(file_path).unwrap()) {
        Ok(atom) => atom,
        Err(e) => {
            eprintln!("Parse error: {e}");
            return;
        }
    };

    let mut scope = Evaluator::new_empty();
    add_host_funcs(&mut scope);

    loop {
        match scope.eval(&atom) {
            Ok(value) => eprint!("\x1b[2K{value}\r"),
            Err(e) => {
                eprintln!("Eval error: {e}");
                return;
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        repl();
    } else {
        let file_path = args.last().expect("at least one arg");

        if args.contains(&String::from("--loop")) {
            exec_file_loop(file_path);
        } else {
            exec_file_once(file_path);
        }
    }
}
