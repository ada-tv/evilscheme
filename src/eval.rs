#![allow(dead_code)]

use std::collections::HashMap;

use crate::Atom;

pub type HostFunction = Box<dyn Fn(&[Atom]) -> Result<Atom, EvalError>>;

#[derive(Debug, PartialEq, Eq)]
pub enum EvalError {
    UnboundVariable(String),
    OutOfBounds(usize),
    TypeMismatch(&'static str),
    SyntaxError(&'static str),
    TooDeep(usize),
    ArityMismatch { expected: usize, got: usize },
}

impl std::fmt::Display for EvalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnboundVariable(name) => write!(f, "undefined symbol {name}"),
            Self::OutOfBounds(n) => write!(f, "out of bounds list access at index {n}"),
            Self::TypeMismatch(msg) => f.write_str(msg),
            Self::SyntaxError(msg) => f.write_str(msg),
            Self::TooDeep(_) => write!(f, "too many nested function calls"),
            Self::ArityMismatch { expected, got } => {
                write!(f, "function arity mismatch, expected {expected}, got {got}")
            }
        }
    }
}

pub struct Evaluator {
    bindings: Vec<HashMap<String, Atom>>,
    host_funcs: Vec<HostFunction>,
}

impl Evaluator {
    const MAX_CALL_DEPTH: usize = 8;

    pub fn new_empty() -> Self {
        Self::new(HashMap::new())
    }

    pub fn new(top_bindings: HashMap<String, Atom>) -> Self {
        let mut tmp = Self {
            bindings: vec![top_bindings],
            host_funcs: Vec::new(),
        };

        for (name, func) in BUILTINS {
            tmp.bind_builtin(name, *func);
        }

        tmp
    }

    fn bind_builtin(&mut self, name: &'static str, func: fn(&[Atom]) -> Result<Atom, EvalError>) {
        self.bind_host_func(name.to_string(), Box::new(func));
    }

    pub fn bind_host_func(&mut self, name: String, func: HostFunction) {
        let idx = self.host_funcs.len();
        self.host_funcs.push(func);
        self.bindings[0].insert(name, Atom::HostFunction(idx));
    }

    fn get_scoped_binding(&self, name: &str) -> Option<Atom> {
        for scope in self.bindings.iter().rev() {
            if let Some(var) = scope.get(name) {
                return Some(var.clone());
            }
        }

        None
    }

    fn set_scoped_binding(&mut self, name: String, value: Atom) -> Result<(), EvalError> {
        for scope in self.bindings.iter_mut().rev() {
            if let Some(e) = scope.get_mut(&name) {
                *e = value;
                return Ok(());
            }
        }

        Err(EvalError::UnboundVariable(name))
    }

    fn set_top_binding(&mut self, name: String, value: Atom) {
        self.bindings
            .last_mut()
            .expect("always at least one scope")
            .insert(name, value);
    }

    fn push_scope(&mut self) {
        self.bindings.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.bindings.pop();
    }

    /*fn builtin_func(&mut self, name: &str, args: &[Atom]) -> Result<Atom, EvalError> {
        match name {
            "+" => {
                let mut accum = 0.0;

                for arg in args {
                    let Atom::Number(num) = arg else {
                        return Err(EvalError::TypeMismatch("+ only works on numbers"));
                    };

                    accum += num;
                }

                Ok(Atom::Number(accum))
            }

            "-" => {
                if args.is_empty() {
                    return Err(EvalError::TypeMismatch("- requires at least one number"));
                }

                let Atom::Number(mut accum) = args[0] else {
                    return Err(EvalError::TypeMismatch("- only works on numbers"));
                };

                if args.len() == 1 {
                    Ok(Atom::Number(-accum))
                } else {
                    for arg in &args[1..] {
                        let Atom::Number(num) = arg else {
                            return Err(EvalError::TypeMismatch("- only works on numbers"));
                        };

                        accum -= num;
                    }

                    Ok(Atom::Number(accum))
                }
            }

            "*" => {
                let mut accum = 1.0;

                for arg in args {
                    let Atom::Number(num) = arg else {
                        return Err(EvalError::TypeMismatch("* only works on numbers"));
                    };

                    accum *= num;
                }

                Ok(Atom::Number(accum))
            }

            "/" => {
                if args.is_empty() {
                    return Err(EvalError::TypeMismatch("/ requires at least one number"));
                }

                let Atom::Number(mut accum) = args[0] else {
                    return Err(EvalError::TypeMismatch("/ only works on numbers"));
                };

                if args.len() == 1 {
                    Ok(Atom::Number(1.0 / accum))
                } else {
                    for arg in &args[1..] {
                        let Atom::Number(num) = arg else {
                            return Err(EvalError::TypeMismatch("/ only works on numbers"));
                        };

                        accum /= num;
                    }

                    Ok(Atom::Number(accum))
                }
            }

            "modulo" => {
                if args.len() != 2 {
                    return Err(EvalError::ArityMismatch {
                        expected: 2,
                        got: args.len(),
                    });
                }

                let Atom::Number(lhs) = args[0] else {
                    return Err(EvalError::TypeMismatch("remainder only works on numbers"));
                };

                let Atom::Number(rhs) = args[1] else {
                    return Err(EvalError::TypeMismatch("remainder only works on numbers"));
                };

                Ok(Atom::Number(lhs.rem_euclid(rhs)))
            }

            "remainder" => {
                if args.len() != 2 {
                    return Err(EvalError::ArityMismatch {
                        expected: 2,
                        got: args.len(),
                    });
                }

                let Atom::Number(lhs) = args[0] else {
                    return Err(EvalError::TypeMismatch("remainder only works on numbers"));
                };

                let Atom::Number(rhs) = args[1] else {
                    return Err(EvalError::TypeMismatch("remainder only works on numbers"));
                };

                Ok(Atom::Number(lhs % rhs))
            }

            "not" => {
                if args.len() != 1 {
                    return Err(EvalError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    });
                }

                if let Atom::Bool(false) = args[0] {
                    Ok(Atom::Bool(true))
                } else {
                    Ok(Atom::Bool(false))
                }
            }

            "null?" => {
                if args.is_empty() {
                    return Err(EvalError::TypeMismatch(
                        "null? requires at least one argument",
                    ));
                }

                if let Atom::Nil = args[0] {
                    Ok(Atom::Bool(true))
                } else {
                    Ok(Atom::Bool(false))
                }
            }

            "number?" => {
                if args.is_empty() {
                    return Err(EvalError::TypeMismatch(
                        "number? requires at least one argument",
                    ));
                }

                if let Atom::Number(_) = args[0] {
                    Ok(Atom::Bool(true))
                } else {
                    Ok(Atom::Bool(false))
                }
            }

            "string?" => {
                if args.is_empty() {
                    return Err(EvalError::TypeMismatch(
                        "string? requires at least one argument",
                    ));
                }

                if let Atom::String(_) = args[0] {
                    Ok(Atom::Bool(true))
                } else {
                    Ok(Atom::Bool(false))
                }
            }

            "nth" => {
                if args.len() < 2 {
                    return Err(EvalError::TypeMismatch("nth needs index and list"));
                }

                let Atom::Number(index) = args[0] else {
                    return Err(EvalError::TypeMismatch("nth index must be number"));
                };

                let Atom::List(ref list) = args[1] else {
                    return Err(EvalError::TypeMismatch("nth list must be list"));
                };

                if !index.is_finite() || index < 0.0 || index > usize::MAX as f64 {
                    return Err(EvalError::OutOfBounds(index.floor() as usize));
                }

                let index = index.floor() as usize;

                if let Some(elem) = list.get(index) {
                    Ok(elem.clone())
                } else {
                    Err(EvalError::OutOfBounds(index))
                }
            }

            "eval" => {
                if args.len() != 1 {
                    return Err(EvalError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    });
                }

                self.eval(&args[0])
            }

            "display" => {
                if args.len() != 1 {
                    return Err(EvalError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    });
                }

                print!("{}", args[0]);
                Ok(Atom::Nil)
            }

            // FIXME: '("a b" c) writes (a b c)
            "write" => {
                if args.len() != 1 {
                    return Err(EvalError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    });
                }

                if let Atom::String(arg) = &args[0] {
                    print!("{:?}", arg);
                } else if let Atom::HostFunction(func) = args[0] {
                    for (name, atom) in &self.bindings[0] {
                        match atom {
                            Atom::HostFunction(bound_func) if *bound_func == func => {
                                print!("{}", name)
                            }
                            _ => {}
                        }
                    }
                } else {
                    print!("{}", args[0]);
                }

                Ok(Atom::Nil)
            }

            "newline" => {
                println!();
                Ok(Atom::Nil)
            }

            "print" => {
                if args.is_empty() {
                    println!();
                    return Ok(Atom::Nil);
                }

                for arg in &args[..args.len() - 1] {
                    print!("{} ", arg);
                }

                println!("{}", args.last().expect("at least one arg"));

                Ok(Atom::Nil)
            }

            "first" | "car" => {
                if args.len() != 1 {
                    return Err(EvalError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    });
                }

                let Atom::List(ref list) = args[0] else {
                    return Err(EvalError::TypeMismatch("first needs list argument"));
                };

                Ok(list[0].clone())
            }

            "last" => {
                if args.len() != 1 {
                    return Err(EvalError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    });
                }

                let Atom::List(ref list) = args[0] else {
                    return Err(EvalError::TypeMismatch("last needs list argument"));
                };

                Ok(list[list.len().saturating_sub(1)].clone())
            }

            "rest" | "cdr" => {
                if args.len() != 1 {
                    return Err(EvalError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    });
                }

                let Atom::List(ref list) = args[0] else {
                    return Err(EvalError::TypeMismatch("rest needs list argument"));
                };

                Ok(Atom::List(list[1..].to_vec()))
            }

            "list" => {
                let mut parts = Vec::new();

                for part in args {
                    parts.push(self.eval(part)?);
                }

                Ok(Atom::List(parts))
            }

            "sin" => {
                let Ok(val) = Self::apply_number_op(args, |n| n.sin()) else {
                    return Err(EvalError::TypeMismatch("sin only works on numbers"));
                };

                Ok(val)
            }

            "cos" => {
                let Ok(val) = Self::apply_number_op(args, |n| n.cos()) else {
                    return Err(EvalError::TypeMismatch("cos only works on numbers"));
                };

                Ok(val)
            }

            "sqrt" => {
                let Ok(val) = Self::apply_number_op(args, |n| n.sqrt()) else {
                    return Err(EvalError::TypeMismatch("sqrt only works on numbers"));
                };

                Ok(val)
            }

            "round" => {
                let Ok(val) = Self::apply_number_op(args, |n| n.round()) else {
                    return Err(EvalError::TypeMismatch("round only works on numbers"));
                };

                Ok(val)
            }

            "ceil" => {
                let Ok(val) = Self::apply_number_op(args, |n| n.ceil()) else {
                    return Err(EvalError::TypeMismatch("ceil only works on numbers"));
                };

                Ok(val)
            }

            "floor" => {
                let Ok(val) = Self::apply_number_op(args, |n| n.floor()) else {
                    return Err(EvalError::TypeMismatch("floor only works on numbers"));
                };

                Ok(val)
            }

            "trunc" => {
                let Ok(val) = Self::apply_number_op(args, |n| n.trunc()) else {
                    return Err(EvalError::TypeMismatch("trunc only works on numbers"));
                };

                Ok(val)
            }

            _ => Err(EvalError::UnboundVariable(name.into())),
        }
    }*/

    fn eval_inner(&mut self, atom: &Atom, call_depth: usize) -> Result<Atom, EvalError> {
        if call_depth >= Self::MAX_CALL_DEPTH {
            return Err(EvalError::TooDeep(call_depth));
        }

        match atom {
            // ()
            Atom::List(x) if x.is_empty() => Ok(Atom::Nil),

            // (set! symbol any)
            Atom::List(list) if list[0] == Atom::Symbol("set!".into()) => {
                if list.len() < 3 {
                    return Err(EvalError::SyntaxError("set! requires binding and value"));
                }

                if let Atom::Symbol(sym) = &list[1] {
                    let value = self.eval_inner(&list[2], call_depth + 1)?;
                    self.set_scoped_binding(sym.clone(), value)?;
                    Ok(Atom::Nil)
                } else {
                    Err(EvalError::SyntaxError("set! requires binding and value"))
                }
            }

            // (define symbol any)
            // (define (symbol...) any...)
            Atom::List(list) if list[0] == Atom::Symbol("define".into()) => {
                if list.len() < 3 {
                    return Err(EvalError::SyntaxError("define requires binding and value"));
                }

                if let Atom::Symbol(sym) = &list[1] {
                    let value = self.eval_inner(&list[2], call_depth + 1)?;
                    self.set_top_binding(sym.clone(), value);

                    Ok(Atom::Nil)
                } else if let Atom::List(args) = &list[1] {
                    let Some(Atom::Symbol(name)) = args.first() else {
                        return Err(EvalError::SyntaxError("define binding must be symbol"));
                    };

                    let mut func_args = Vec::new();

                    for arg in &args[1..] {
                        let Atom::Symbol(arg) = arg else {
                            return Err(EvalError::SyntaxError(
                                "function define arguments must be symbol",
                            ));
                        };

                        func_args.push(arg.clone());
                    }

                    let body = &list[2..];

                    self.set_top_binding(
                        name.clone(),
                        if body.len() == 1 {
                            Atom::Function(func_args, Box::new(body[0].clone()))
                        } else {
                            Atom::Function(func_args, Box::new(Atom::List(body.to_vec())))
                        },
                    );

                    Ok(Atom::Nil)
                } else {
                    Err(EvalError::SyntaxError(
                        "define binding must be symbol or (symbol...)",
                    ))
                }
            }

            // (lambda (symbol...) any...)
            Atom::List(list) if list[0] == Atom::Symbol("lambda".into()) => {
                if list.len() < 3 {
                    return Err(EvalError::SyntaxError("lambda requires arguments and body"));
                }

                let Atom::List(arg_syms) = &list[1] else {
                    return Err(EvalError::TypeMismatch(
                        "lambda arguments must be (symbol...)",
                    ));
                };

                let body = &list[2..];

                let mut args = Vec::new();

                for arg in arg_syms {
                    let Atom::Symbol(arg) = arg else {
                        return Err(EvalError::TypeMismatch(
                            "lambda arguments must be (symbol...)",
                        ));
                    };

                    args.push(arg.clone());
                }

                if body.len() == 1 {
                    Ok(Atom::Function(args, Box::new(body[0].clone())))
                } else {
                    Ok(Atom::Function(args, Box::new(Atom::List(body.to_vec()))))
                }
            }

            // (let ((symbol any)...) any)
            Atom::List(list) if list[0] == Atom::Symbol("let".into()) => {
                if list.len() < 3 {
                    return Err(EvalError::SyntaxError("let requires bindings and body"));
                }

                let Atom::List(bindings) = &list[1] else {
                    return Err(EvalError::TypeMismatch("let bindings must be (symbol any)"));
                };

                let mut evalled_bindings = Vec::new();

                for pair in bindings {
                    let Atom::List(pair) = pair else {
                        return Err(EvalError::TypeMismatch("let bindings must be (symbol any)"));
                    };

                    if pair.len() < 2 {
                        return Err(EvalError::TypeMismatch("let bindings must be (symbol any)"));
                    }

                    let Atom::Symbol(ref name) = pair[0] else {
                        return Err(EvalError::TypeMismatch("let bindings must be (symbol any)"));
                    };

                    let value = match self.eval_inner(&pair[1], call_depth + 1) {
                        Ok(x) => x,
                        Err(e) => {
                            self.pop_scope();
                            return Err(e);
                        }
                    };

                    evalled_bindings.push((name.clone(), value));
                }

                self.push_scope();

                for pair in evalled_bindings {
                    self.set_top_binding(pair.0, pair.1);
                }

                let result = self.eval_inner(&list[2], call_depth + 1);

                self.pop_scope();

                result
            }

            // (let* ((symbol any)...) any)
            Atom::List(list) if list[0] == Atom::Symbol("let*".into()) => {
                if list.len() < 3 {
                    return Err(EvalError::SyntaxError("let* requires bindings and body"));
                }

                let Atom::List(bindings) = &list[1] else {
                    return Err(EvalError::TypeMismatch(
                        "let* bindings must be (symbol any)",
                    ));
                };

                self.push_scope();

                for pair in bindings {
                    let Atom::List(pair) = pair else {
                        return Err(EvalError::TypeMismatch(
                            "let* bindings must be (symbol any)",
                        ));
                    };

                    if pair.len() < 2 {
                        return Err(EvalError::TypeMismatch(
                            "let* bindings must be (symbol any)",
                        ));
                    }

                    let Atom::Symbol(ref name) = pair[0] else {
                        return Err(EvalError::TypeMismatch(
                            "let* bindings must be (symbol any)",
                        ));
                    };

                    let value = match self.eval_inner(&pair[1], call_depth + 1) {
                        Ok(x) => x,
                        Err(e) => {
                            self.pop_scope();
                            return Err(e);
                        }
                    };

                    self.set_top_binding(name.clone(), value);
                }

                let result = self.eval_inner(&list[2], call_depth + 1);

                self.pop_scope();

                result
            }

            // (if bool any any?)
            Atom::List(list) if list[0] == Atom::Symbol("if".into()) => {
                if list.len() < 3 || list.len() > 4 {
                    return Err(EvalError::SyntaxError(
                        "if requires condition and true-body",
                    ));
                }

                let cond = if let Atom::Bool(false) = self.eval_inner(&list[1], call_depth + 1)? {
                    false
                } else {
                    true
                };

                if cond {
                    self.eval_inner(&list[2], call_depth + 1)
                } else if list.len() == 4 {
                    self.eval_inner(&list[3], call_depth + 1)
                } else {
                    Ok(Atom::Nil)
                }
            }

            // TODO: (and) with short circuiting and falsey conversion
            // TODO: (or) with short circuiting and falsey conversion

            // scripted function calls
            Atom::List(list) if let Atom::Function(bindings, body) = &list[0] => {
                let args = &list[1..];

                if args.len() != bindings.len() {
                    return Err(EvalError::ArityMismatch {
                        expected: bindings.len(),
                        got: args.len(),
                    });
                }

                self.push_scope();

                for (name, value) in std::iter::zip(bindings, args) {
                    let value = self.eval_inner(value, call_depth + 1)?;
                    self.set_top_binding(name.clone(), value);
                }

                let result = match self.eval_inner(body, call_depth + 1) {
                    Ok(Atom::List(list)) if list.is_empty() => Ok(Atom::Nil),
                    Ok(Atom::List(list)) => Ok(list.last().expect("at least one element").clone()),
                    x => x,
                };

                self.pop_scope();

                result
            }

            // host function calls
            Atom::List(list) if let Atom::HostFunction(func) = &list[0] => {
                let mut args = Vec::new();

                for arg in &list[1..] {
                    args.push(self.eval_inner(arg, call_depth + 1)?);
                }

                if let Some(func) = self.host_funcs.get(*func) {
                    func(&args)
                } else {
                    Err(EvalError::UnboundVariable(format!("builtin {func}")))
                }
            }

            // FIXME: this feels wrong
            Atom::List(list) if let Atom::List(_) = list[0] => {
                let mut parts = vec![self.eval_inner(&list[0], call_depth + 1)?];
                parts.extend_from_slice(&list[1..]);

                self.eval_inner(&Atom::List(parts), call_depth + 1)
            }

            Atom::List(list) if let Atom::Symbol(_) = list[0] => {
                let mut parts = vec![self.eval_inner(&list[0], call_depth + 1)?];
                parts.extend_from_slice(&list[1..]);

                self.eval_inner(&Atom::List(parts), call_depth + 1)
            }

            Atom::List(_) => Err(EvalError::TypeMismatch(
                "non-executable list called as function",
            )),

            Atom::Symbol(sym) => {
                if let Some(val) = self.get_scoped_binding(sym) {
                    Ok(val.clone())
                } else {
                    Err(EvalError::UnboundVariable(sym.clone()))
                }
            }

            Atom::Quote(atom) => Ok(*atom.clone()),

            _ => Ok(atom.clone()),
        }
    }

    pub fn eval(&mut self, atom: &Atom) -> Result<Atom, EvalError> {
        match self.eval_inner(atom, 0) {
            Ok(Atom::List(list)) if list.is_empty() => Ok(Atom::Nil),
            Ok(Atom::List(list)) => Ok(list.last().expect("at least one element").clone()),
            result => result,
        }
    }
}

const BUILTINS: &[(&str, fn(&[Atom]) -> Result<Atom, EvalError>)] = &[
    ("+", |args| {
        let mut sum = 0.0;

        for arg in args {
            let Atom::Number(arg) = arg else {
                return Err(EvalError::TypeMismatch("+ only takes number"));
            };

            sum += arg;
        }

        Ok(Atom::Number(sum))
    }),
    ("*", |args| {
        let mut sum = 1.0;

        for arg in args {
            let Atom::Number(arg) = arg else {
                return Err(EvalError::TypeMismatch("* only takes number"));
            };

            sum *= arg;
        }

        Ok(Atom::Number(sum))
    }),
    ("-", |args| {
        if args.is_empty() {
            return Err(EvalError::ArityMismatch {
                expected: 1,
                got: 0,
            });
        }

        let Atom::Number(first) = args[0] else {
            return Err(EvalError::TypeMismatch("- only takes number"));
        };

        if args.len() == 1 {
            Ok(Atom::Number(-first))
        } else {
            let mut sum = first;

            for arg in &args[1..] {
                let Atom::Number(arg) = arg else {
                    return Err(EvalError::TypeMismatch("- only takes number"));
                };

                sum -= arg;
            }

            Ok(Atom::Number(sum))
        }
    }),
    ("/", |args| {
        if args.is_empty() {
            return Err(EvalError::ArityMismatch {
                expected: 1,
                got: 0,
            });
        }

        let Atom::Number(first) = args[0] else {
            return Err(EvalError::TypeMismatch("/ only takes number"));
        };

        if args.len() == 1 {
            Ok(Atom::Number(1.0 / first))
        } else {
            let mut sum = first;

            for arg in &args[1..] {
                let Atom::Number(arg) = arg else {
                    return Err(EvalError::TypeMismatch("/ only takes number"));
                };

                sum /= arg;
            }

            Ok(Atom::Number(sum))
        }
    }),
    ("equal?", |args| {
        if args.len() < 2 {
            return Err(EvalError::ArityMismatch {
                expected: 2,
                got: args.len(),
            });
        }

        let first = &args[0];

        let mut sum = true;

        for arg in &args[1..] {
            sum = sum && (first == arg);
        }

        Ok(Atom::Bool(sum))
    }),
    ("=", |args| {
        if args.len() < 2 {
            return Err(EvalError::ArityMismatch {
                expected: 2,
                got: args.len(),
            });
        }

        let Atom::Number(first) = args[0] else {
            return Err(EvalError::TypeMismatch("= only takes number, try equal?"));
        };

        let mut sum = true;

        for arg in &args[1..] {
            let Atom::Number(arg) = arg else {
                return Err(EvalError::TypeMismatch("= only takes number, try equal?"));
            };

            sum = sum && (first == *arg);
        }

        Ok(Atom::Bool(sum))
    }),
    ("<", |args| {
        if args.len() < 2 {
            return Err(EvalError::ArityMismatch {
                expected: 2,
                got: args.len(),
            });
        }

        let Atom::Number(first) = args[0] else {
            return Err(EvalError::TypeMismatch("< only takes number"));
        };

        let mut sum = true;

        for arg in &args[1..] {
            let Atom::Number(arg) = arg else {
                return Err(EvalError::TypeMismatch("< only takes number"));
            };

            sum = sum && (first < *arg);
        }

        Ok(Atom::Bool(sum))
    }),
    (">", |args| {
        if args.len() < 2 {
            return Err(EvalError::ArityMismatch {
                expected: 2,
                got: args.len(),
            });
        }

        let Atom::Number(first) = args[0] else {
            return Err(EvalError::TypeMismatch("> only takes number"));
        };

        let mut sum = true;

        for arg in &args[1..] {
            let Atom::Number(arg) = arg else {
                return Err(EvalError::TypeMismatch("> only takes number"));
            };

            sum = sum && (first > *arg);
        }

        Ok(Atom::Bool(sum))
    }),
    ("<=", |args| {
        if args.len() < 2 {
            return Err(EvalError::ArityMismatch {
                expected: 2,
                got: args.len(),
            });
        }

        let Atom::Number(first) = args[0] else {
            return Err(EvalError::TypeMismatch("<= only takes number"));
        };

        let mut sum = true;

        for arg in &args[1..] {
            let Atom::Number(arg) = arg else {
                return Err(EvalError::TypeMismatch("<= only takes number"));
            };

            sum = sum && (first <= *arg);
        }

        Ok(Atom::Bool(sum))
    }),
    (">=", |args| {
        if args.len() < 2 {
            return Err(EvalError::ArityMismatch {
                expected: 2,
                got: args.len(),
            });
        }

        let Atom::Number(first) = args[0] else {
            return Err(EvalError::TypeMismatch(">= only takes number"));
        };

        let mut sum = true;

        for arg in &args[1..] {
            let Atom::Number(arg) = arg else {
                return Err(EvalError::TypeMismatch(">= only takes number"));
            };

            sum = sum && (first >= *arg);
        }

        Ok(Atom::Bool(sum))
    }),
    ("floor", |args| {
        if args.len() != 1 {
            return Err(EvalError::ArityMismatch {
                expected: 1,
                got: 0,
            });
        }

        let Atom::Number(lhs) = args[0] else {
            return Err(EvalError::TypeMismatch("floor only takes number"));
        };

        Ok(Atom::Number(lhs.floor()))
    }),
    ("ceiling", |args| {
        if args.len() != 1 {
            return Err(EvalError::ArityMismatch {
                expected: 1,
                got: 0,
            });
        }

        let Atom::Number(lhs) = args[0] else {
            return Err(EvalError::TypeMismatch("ceiling only takes number"));
        };

        Ok(Atom::Number(lhs.ceil()))
    }),
    ("truncate", |args| {
        if args.len() != 1 {
            return Err(EvalError::ArityMismatch {
                expected: 1,
                got: 0,
            });
        }

        let Atom::Number(lhs) = args[0] else {
            return Err(EvalError::TypeMismatch("truncate only takes number"));
        };

        Ok(Atom::Number(lhs.trunc()))
    }),
    ("round", |args| {
        if args.len() != 1 {
            return Err(EvalError::ArityMismatch {
                expected: 1,
                got: 0,
            });
        }

        let Atom::Number(lhs) = args[0] else {
            return Err(EvalError::TypeMismatch("round only takes number"));
        };

        Ok(Atom::Number(lhs.round_ties_even()))
    }),
    ("sqrt", |args| {
        if args.len() != 1 {
            return Err(EvalError::ArityMismatch {
                expected: 1,
                got: 0,
            });
        }

        let Atom::Number(lhs) = args[0] else {
            return Err(EvalError::TypeMismatch("sqrt only takes number"));
        };

        Ok(Atom::Number(lhs.sqrt()))
    }),
    ("number?", |args| {
        if args.len() != 1 {
            return Err(EvalError::ArityMismatch {
                expected: 1,
                got: 0,
            });
        }

        if let Atom::Number(_) = args[0] {
            Ok(Atom::Bool(true))
        } else {
            Ok(Atom::Bool(false))
        }
    }),
    ("integer?", |args| {
        if args.len() != 1 {
            return Err(EvalError::ArityMismatch {
                expected: 1,
                got: 0,
            });
        }

        if let Atom::Number(lhs) = args[0] {
            Ok(Atom::Bool(lhs.fract() == 0.0))
        } else {
            Ok(Atom::Bool(false))
        }
    }),
];

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn lambda() {
        const TEST_SRC: &str = "(define square (lambda (a) (* a a))) (square 4)";
        let atom = Atom::parse(TEST_SRC).unwrap();
        let mut evaluator = Evaluator::new_empty();
        assert_eq!(
            evaluator.eval(&atom),
            Ok(Atom::List(vec![Atom::Nil, Atom::Number(16.0)]))
        );
    }
}
