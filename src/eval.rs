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
            Self::TooDeep(_) => write!(f, "too many nested expressions ({})", Evaluator::MAX_DEPTH),
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
    const MAX_DEPTH: usize = 64;

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

    fn get_in_scope(&self, name: &str) -> Option<Atom> {
        for scope in self.bindings.iter().rev() {
            if let Some(var) = scope.get(name) {
                return Some(var.clone());
            }
        }

        None
    }

    fn try_set_in_any_scope(&mut self, name: String, value: Atom) -> Result<(), EvalError> {
        for scope in self.bindings.iter_mut().rev() {
            if let Some(e) = scope.get_mut(&name) {
                *e = value;
                return Ok(());
            }
        }

        Err(EvalError::UnboundVariable(name))
    }

    pub fn set_in_current_scope(&mut self, name: String, value: Atom) {
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

    fn eval_inner(&mut self, atom: &Atom, call_depth: usize) -> Result<Atom, EvalError> {
        if call_depth >= Self::MAX_DEPTH {
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
                    self.try_set_in_any_scope(sym.clone(), value)?;
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
                    self.set_in_current_scope(sym.clone(), value);

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

                    self.set_in_current_scope(
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
                    self.set_in_current_scope(pair.0, pair.1);
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

                    self.set_in_current_scope(name.clone(), value);
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

            // (cond (bool any)... (else any)?)
            Atom::List(list) if list[0] == Atom::Symbol("cond".into()) => {
                if list.len() < 2 {
                    return Err(EvalError::SyntaxError(
                        "cond requires at least one condition branch",
                    ));
                }

                let mut else_branch = Atom::Nil;

                const LIST_ERROR: EvalError = EvalError::SyntaxError(
                    "cond branches must be a list with a condition and value expression",
                );

                for branch in &list[1..] {
                    let Atom::List(pair) = branch else {
                        return Err(LIST_ERROR);
                    };
                    if pair.len() != 2 {
                        return Err(LIST_ERROR);
                    }

                    if let Atom::Symbol(sym) = &pair[0]
                        && sym == "else"
                    {
                        else_branch = pair[1].clone();
                        continue;
                    }

                    let Atom::Bool(cond) = self.eval_inner(&pair[0], call_depth + 1)? else {
                        return Err(EvalError::SyntaxError("cond branch condition must be bool"));
                    };

                    if cond {
                        return self.eval_inner(&pair[1], call_depth + 1);
                    }
                }

                self.eval_inner(&else_branch, call_depth + 1)
            }

            // (and any...)
            Atom::List(list) if list[0] == Atom::Symbol("and".into()) => {
                let mut value = Atom::Bool(true);

                for arg in &list[1..] {
                    value = self.eval_inner(arg, call_depth + 1)?;

                    if let Atom::Bool(false) = value {
                        return Ok(value);
                    }
                }

                Ok(value)
            }

            // (or any...)
            Atom::List(list) if list[0] == Atom::Symbol("or".into()) => {
                let mut value = Atom::Bool(false);

                for arg in &list[1..] {
                    value = self.eval_inner(arg, call_depth + 1)?;

                    match value {
                        Atom::Bool(false) => {}
                        x => return Ok(x),
                    }
                }

                Ok(value)
            }

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
                    self.set_in_current_scope(name.clone(), value);
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

            // FIXME: this also feels wrong
            Atom::List(list) => {
                let mut value = Atom::Nil;

                for atom in list {
                    value = self.eval_inner(atom, call_depth + 1)?;
                }

                Ok(value)
            }

            Atom::Symbol(sym) => {
                if let Some(val) = self.get_in_scope(sym) {
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
        self.eval_inner(atom, 0)
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
