mod eval;
mod parse;

pub use eval::{EvalError, Scope};
pub use parse::Atom;

pub type HostFunction = Box<dyn Fn(&[Atom]) -> Result<Atom, EvalError>>;
