#![forbid(unsafe_code)]
//! DSS script tokenizer and inline RPN calculator.

pub mod compat;
pub mod parser;
pub mod rpn;
pub mod vars;

pub use parser::{Parser, ParserError, val_f64, val_i32};
pub use rpn::RPNCalculator;
pub use vars::ParserVars;
