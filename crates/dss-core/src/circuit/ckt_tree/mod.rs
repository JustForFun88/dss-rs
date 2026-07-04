//! Port of `Shared/CktTree.pas` — `TCktTree`/`TCktTreeNode`/`TZoneEndsList`
//! plus `BuildActiveBusAdjacencyLists` and its helpers.
//!
//! The Pascal tree is pointer-linked; here it is an index arena
//! (`Vec<TreeNode>` with parent/child indices) and every Pascal object
//! pointer payload becomes an [`ElemRef`]. The traversal machinery
//! (`PushAllChildren`/`GoForward` with the explicit LIFO stack) is ported
//! verbatim because its visit order defines the EnergyMeter `SequenceList`,
//! which is observable (reliability sweeps, zone dumps, reductions).

#[cfg(test)]
mod tests;

use crate::circuit::Circuit;
use crate::elements::traits::{ElemRef, ElemStore};

/// Sentinel for "no bus reference" (Pascal used `0` in its 1-based bus
/// indexing; our bus indices are 0-based, so the sentinel is `usize::MAX`,
/// matching `Terminal::bus_ref`).
pub const NO_BUS: usize = usize::MAX;

/// One tree node (`TCktTreeNode`). Public state mirrors the Pascal fields the
/// EnergyMeter zone machinery reads/writes.
#[derive(Debug, Clone)]
pub struct TreeNode {
    /// `CktObject` — the branch (PD element) this node represents.
    pub elem: ElemRef,
    parent: Option<usize>,
    children: Vec<usize>,
    /// `ChildAdded`: children appended since the node was last expanded.
    child_added: bool,
    /// `LexicalLevel`: root = 0.
    lexical_level: i32,
    /// `FShuntObjects`: loads/generators/shunt caps attached at this node.
    pub shunts: Vec<ElemRef>,
    /// `FromBusReference` (bus index, [`NO_BUS`] = unset).
    pub from_bus: usize,
    /// `VoltBaseIndex` (EnergyMeter voltage-base list slot).
    pub volt_base_index: i32,
    /// `FromTerminal` (1-based).
    pub from_terminal: usize,
    pub is_looped: bool,
    pub is_parallel: bool,
    pub is_dangling: bool,
    /// `LoopLineObj`.
    pub loop_elem: Option<ElemRef>,
    /// `ToBusList` + its sequential-access cursor (`ToBusPtr`).
    to_bus_list: Vec<usize>,
    to_bus_ptr: usize,
}

impl TreeNode {
    fn new(elem: ElemRef, parent: Option<usize>, lexical_level: i32) -> Self {
        Self {
            elem,
            parent,
            children: Vec::new(),
            child_added: false,
            lexical_level,
            shunts: Vec::new(),
            from_bus: NO_BUS,
            volt_base_index: 0,
            from_terminal: 0,
            is_looped: false,
            is_parallel: false,
            // Pascal ctor: `IsDangling := TRUE`.
            is_dangling: true,
            loop_elem: None,
            to_bus_list: Vec::new(),
            to_bus_ptr: 0,
        }
    }

    /// Pascal `Set_ToBusReference`: appends to the to-bus list.
    pub fn add_to_bus_reference(&mut self, bus: usize) {
        self.to_bus_list.push(bus);
    }

    /// Pascal `Get_ToBusReference` — stateful sequential access: with exactly
    /// one entry it always returns it; otherwise each call advances a cursor,
    /// returning `None` (Pascal `-1`) once past the end and resetting for the
    /// next sweep.
    pub fn next_to_bus_reference(&mut self) -> Option<usize> {
        if self.to_bus_list.len() == 1 {
            return Some(self.to_bus_list[0]);
        }
        self.to_bus_ptr += 1;
        if self.to_bus_ptr > self.to_bus_list.len() {
            self.to_bus_ptr = 0; // ready for the next access sequence
            None
        } else {
            Some(self.to_bus_list[self.to_bus_ptr - 1])
        }
    }

    /// Pascal `ResetToBusList`.
    pub fn reset_to_bus_list(&mut self) {
        self.to_bus_ptr = 0;
    }

    /// Pascal `NumChildBranches`.
    pub fn num_child_branches(&self) -> usize {
        self.children.len()
    }

    /// Pascal `NumShuntObjects`.
    pub fn num_shunt_objects(&self) -> usize {
        self.shunts.len()
    }

    /// Child node indices in insertion order (`FirstChildBranch`/
    /// `NextChildBranch` iteration order).
    pub fn children(&self) -> &[usize] {
        &self.children
    }

    /// Pascal `ParentBranch` (node index).
    pub fn parent(&self) -> Option<usize> {
        self.parent
    }

    /// Pascal `TCktTreeNode.LexicalLevel` (== `BranchList.Level` when this node is
    /// the present branch). Root = 0.
    pub fn level(&self) -> i32 {
        self.lexical_level
    }
}

/// Pascal `TZoneEndsList`: the feeder end points — `(tree node index,
/// end bus)` pairs.
#[derive(Debug, Clone, Default)]
pub struct ZoneEndsList {
    pub ends: Vec<(usize, usize)>,
}

impl ZoneEndsList {
    /// Pascal `Add(Node, EndBusRef)`.
    pub fn add(&mut self, node: usize, end_bus: usize) {
        self.ends.push((node, end_bus));
    }

    pub fn num_ends(&self) -> usize {
        self.ends.len()
    }
}

/// Pascal `TCktTree`. `present` is `PresentBranch`.
#[derive(Debug, Clone, Default)]
pub struct CktTree {
    nodes: Vec<TreeNode>,
    first: Option<usize>,
    pub present: Option<usize>,
    forward_stack: Vec<usize>,
    pub zone_ends: ZoneEndsList,
}

impl CktTree {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn node(&self, idx: usize) -> &TreeNode {
        &self.nodes[idx]
    }

    pub fn node_mut(&mut self, idx: usize) -> &mut TreeNode {
        &mut self.nodes[idx]
    }

    /// The present branch (panics when there is none — callers check first,
    /// exactly where Pascal would have dereferenced NIL).
    pub fn present_node(&self) -> &TreeNode {
        &self.nodes[self.present.expect("CktTree: no present branch")]
    }

    pub fn present_node_mut(&mut self) -> &mut TreeNode {
        let i = self.present.expect("CktTree: no present branch");
        &mut self.nodes[i]
    }

    /// Pascal `Add`: new node becomes the present branch, parented to the old
    /// present branch — but **not** entered in the parent's child list (only
    /// `AddNewChild` does that). Used for the root.
    pub fn add(&mut self, elem: ElemRef) -> usize {
        let level = self.present.map_or(0, |p| self.nodes[p].lexical_level + 1);
        let idx = self.nodes.len();
        self.nodes.push(TreeNode::new(elem, self.present, level));
        self.present = Some(idx);
        if self.first.is_none() {
            self.first = Some(idx);
        }
        idx
    }

    /// Pascal `AddNewChild`: append a child to the present branch (present
    /// does not move). With no present branch it degenerates to `Add`.
    pub fn add_new_child(&mut self, elem: ElemRef, bus_ref: usize, terminal_no: usize) -> usize {
        let Some(parent) = self.present else {
            return self.add(elem);
        };
        let level = self.nodes[parent].lexical_level + 1;
        let idx = self.nodes.len();
        let mut node = TreeNode::new(elem, Some(parent), level);
        node.from_bus = bus_ref;
        node.from_terminal = terminal_no;
        self.nodes.push(node);
        self.nodes[parent].children.push(idx);
        self.nodes[parent].child_added = true;
        idx
    }

    /// Pascal `AddNewObject`: attach a shunt object to the present branch.
    pub fn add_new_object(&mut self, elem: ElemRef) {
        if let Some(p) = self.present {
            self.nodes[p].shunts.push(elem);
        }
    }

    /// Pascal `PushAllChildren`: push the present branch's children onto the
    /// forward stack (insertion order, so the **last** child pops first) and
    /// clear `ChildAdded`.
    fn push_all_children(&mut self) {
        let Some(p) = self.present else {
            return;
        };
        for i in 0..self.nodes[p].children.len() {
            let c = self.nodes[p].children[i];
            self.forward_stack.push(c);
        }
        self.nodes[p].child_added = false;
    }

    /// Pascal `GoForward`: stack-driven traversal; returns the new present
    /// branch's element (`None` ends the sweep).
    pub fn go_forward(&mut self) -> Option<ElemRef> {
        // If we have added children to the present node since we opened it,
        // push them on.
        if let Some(p) = self.present
            && self.nodes[p].child_added
        {
            self.push_all_children();
        }
        // If the forward stack is empty, push stuff on it to get started.
        if self.forward_stack.is_empty() {
            self.push_all_children();
        }
        self.present = self.forward_stack.pop();
        self.push_all_children(); // push all children of latest
        self.present.map(|i| self.nodes[i].elem)
    }

    /// Pascal `GoBackward`: move to the parent and reset the forward stack.
    pub fn go_backward(&mut self) -> Option<ElemRef> {
        let p = self.present?;
        self.present = self.nodes[p].parent;
        self.forward_stack.clear();
        self.present.map(|i| self.nodes[i].elem)
    }

    /// Pascal `Parent`: the present branch's parent element.
    pub fn parent(&self) -> Option<ElemRef> {
        let p = self.present?;
        self.nodes[p].parent.map(|i| self.nodes[i].elem)
    }

    /// Pascal `First`: go to the beginning, reset the stack, prime the
    /// traversal.
    pub fn first(&mut self) -> Option<ElemRef> {
        self.present = self.first;
        self.forward_stack.clear();
        self.push_all_children();
        self.present.map(|i| self.nodes[i].elem)
    }

    /// Pascal `Active`.
    pub fn active(&self) -> Option<ElemRef> {
        self.present.map(|i| self.nodes[i].elem)
    }

    /// Pascal `StartHere`: restart the forward search at the present branch.
    pub fn start_here(&mut self) {
        self.forward_stack.clear();
        if let Some(p) = self.present {
            self.forward_stack.push(p);
        }
    }

    /// Pascal `Level`.
    pub fn level(&self) -> i32 {
        self.present.map_or(0, |i| self.nodes[i].lexical_level)
    }
}

/// Bus adjacency lists (`TAdjArray` pair): for each bus index, the enabled
/// elements connected there. `pd` holds non-shunt PD branches (every
/// terminal's bus); `pc` holds PC elements **plus shunt PD elements**
/// (terminal-1 bus only), exactly like the Pascal lists.
#[derive(Debug, Clone, Default)]
pub struct BusAdjLists {
    pub pd: Vec<Vec<ElemRef>>,
    pub pc: Vec<Vec<ElemRef>>,
}

/// Pascal `AllTerminalsClosed`: at least one of the first `nphases`
/// conductors closed on **every** terminal.
pub fn all_terminals_closed(elem: &dyn crate::elements::traits::CktElement) -> bool {
    let cd = elem.cd();
    if cd.terminals.is_empty() {
        return false;
    }
    for term in &cd.terminals {
        let closed_one = term.conductors_closed.iter().take(cd.nphases).any(|&c| c);
        if !closed_one {
            return false;
        }
    }
    true
}

/// Pascal `BuildActiveBusAdjacencyLists` (CktTree.pas l.678): walk the
/// circuit's PC and PD lists and bucket enabled elements by terminal bus.
pub fn build_active_bus_adjacency_lists(ckt: &Circuit, store: &dyn ElemStore) -> BusAdjLists {
    let n_bus = ckt.buses.len();
    let mut adj = BusAdjLists {
        pd: vec![Vec::new(); n_bus],
        pc: vec![Vec::new(); n_bus],
    };

    for &r in &ckt.pc_elements {
        let elem = store.ckt_elem(r);
        if elem.cd().enabled {
            let i = elem.cd().terminals[0].bus_ref;
            if i < n_bus {
                adj.pc[i].push(r);
            }
        }
    }

    // Put only eligible PD elements in the list.
    for &r in &ckt.pd_elements {
        let elem = store.ckt_elem(r);
        if !elem.cd().enabled {
            continue;
        }
        if elem.is_shunt() {
            // Shunt capacitors/reactors go on the PC list (terminal 1).
            let i = elem.cd().terminals[0].bus_ref;
            if i < n_bus {
                adj.pc[i].push(r);
            }
        } else if all_terminals_closed(elem) {
            for term in &elem.cd().terminals {
                let i = term.bus_ref;
                if i < n_bus {
                    adj.pd[i].push(r);
                }
            }
        }
    }
    adj
}
