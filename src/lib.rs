mod eval;
mod parse;

pub use eval::{EvalError, Evaluator};
pub use parse::Atom;

pub type HostFunction = Box<dyn Fn(&[Atom]) -> Result<Atom, EvalError>>;
