//! Circuit model: `Circuit.pas`, `Bus.pas`, `Terminal.pas`.
#![allow(clippy::module_inception)]

pub mod auto_add;
pub mod bus;
pub mod circuit;
pub mod ckt_tree;
pub mod coverage;
pub mod tearing;
pub mod terminal;

pub use auto_add::{AutoAdd, CAPADD, GENADD};
pub use bus::Bus;
pub use circuit::{BusMarker, Circuit, ElemKind, NodeBus, ReductionStrategy};
pub use ckt_tree::{BusAdjLists, CktTree, TreeNode, ZoneEndsList};
pub use tearing::AdTearing;
pub use terminal::Terminal;
