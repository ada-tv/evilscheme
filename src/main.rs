mod eval;
mod parse;

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use evilscheme::{Atom, Evaluator};

fn repl() {
    let stdin = std::io::stdin();
    let mut scope = Evaluator::new_empty();

    loop {
        eprint!("#> ");

        let mut src = String::new();
        stdin.read_line(&mut src).unwrap();

        if src.len() == 0 {
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

        let result = match scope.eval(&atom) {
            Ok(a) => a,
            Err(e) => {
                eprintln!(";; Eval error: {}", e);
                continue;
            }
        };

        println!("{}", result);
    }
}

fn exec_file_once(file_path: &str) {
    let atom = Atom::parse(&std::fs::read_to_string(file_path).unwrap()).unwrap();
    let mut eval = Evaluator::new_empty();

    println!("{}", eval.eval(&atom).unwrap());
}

fn exec_file_loop(file_path: &str) {
    let atom = Atom::parse(&std::fs::read_to_string(file_path).unwrap()).unwrap();

    let start = std::time::Instant::now();

    loop {
        let mut eval = Evaluator::new(HashMap::from([(
            "time".into(),
            Atom::Number(Instant::now().duration_since(start).as_secs_f64()),
        )]));

        eprint!("\x1b[2K{}\r", eval.eval(&atom).unwrap());

        std::thread::sleep(Duration::from_millis(50));
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        repl();
    } else {
        let file_path = &args[args.len() - 1];

        if args.contains(&String::from("--loop")) {
            exec_file_loop(file_path);
        } else {
            exec_file_once(file_path);
        }
    }
}
