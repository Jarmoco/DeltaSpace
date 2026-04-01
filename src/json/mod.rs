/* -----------------------------------------------------------------------------
 * json/mod.rs
 * Re-exports the JSON Value type and parse/stringify functions as the
 * public API for the json module.
 * -------------------------------------------------------------------------- */

mod parser;
mod value;

// Exports

pub use parser::{parse, stringify, ParseError, Result};
pub use value::Value;
