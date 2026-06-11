#![forbid(unsafe_code)]
//! DSS script tokenizer and inline RPN calculator, port of the Pascal
//! `src/Parser` directory (`ParserDel.pas`, `RPN.pas`) from
//! `.inputs/dss_capi`. See PORTING_PLAN.md at the repository root.

pub mod parser;
pub mod rpn;
pub mod vars;

pub use parser::{Parser, ParserError, val_f64, val_i32};
pub use rpn::RPNCalculator;
pub use vars::ParserVars;
