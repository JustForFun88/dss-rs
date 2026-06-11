//! Port of `Common/Circuit.pas` (`TDSSCircuit`), Phase 3 subset:
//! `AddCktElement`, `AddBus`, `ProcessBusDefs`, `ReprocessBusDefs`, the
//! bus/node mapping and the circuit-level solve settings.

use num_complex::Complex64;

use dss_parser::{Parser, ParserVars};

use crate::circuit::bus::Bus;
use crate::elements::traits::{CktElement, ElemRef, ElemStore};
use crate::solution::Solution;
use crate::support::hashlist::HashList;

/// Pascal `TNodeBus`: global node number → (bus, user node number).
#[derive(Debug, Clone, Copy, Default)]
pub struct NodeBus {
    /// Index into `buses` (0-based here; Pascal was 1-based).
    pub bus_ref: usize,
    pub node_num: i32,
}

/// Which class-specific list an element joins in `AddCktElement`
/// (the `DSSObjType and CLASSMASK` dispatch, Phase 3 subset).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElemKind {
    Source,
    Line,
    Load,
    Transformer,
}

/// The circuit model (`TDSSCircuit`).
pub struct Circuit {
    /// Lowercased circuit name.
    pub name: String,
    pub case_name: String,
    /// Bus name list; index aligns with `buses` (0-based).
    pub bus_list: HashList,
    pub buses: Vec<Bus>,
    /// Global node count (node references are 1..=num_nodes; 0 = ground).
    pub num_nodes: usize,
    /// `MapNodeToBus[1..num_nodes]`; slot 0 unused.
    pub map_node_to_bus: Vec<NodeBus>,
    /// Device name list, in creation order (`DeviceList`).
    pub device_list: HashList,
    pub num_devices: usize,

    /// `CktElements` in creation order, plus the per-kind lists.
    pub ckt_elements: Vec<ElemRef>,
    pub pd_elements: Vec<ElemRef>,
    pub pc_elements: Vec<ElemRef>,
    pub sources: Vec<ElemRef>,
    pub lines: Vec<ElemRef>,
    pub loads: Vec<ElemRef>,
    pub transformers: Vec<ElemRef>,

    pub solution: Solution,

    pub fundamental: f64,
    pub is_solved: bool,
    pub bus_name_redefined: bool,
    pub solution_was_attempted: bool,

    pub load_multiplier: f64,
    pub gen_multiplier: f64,
    pub default_growth_rate: f64,
    pub default_growth_factor: f64,
    pub positive_sequence: bool,
    pub neglect_load_y: bool,
    pub long_line_correction: bool,
    pub duplicates_allowed: bool,
    pub zones_locked: bool,
    pub meter_zones_computed: bool,
    pub log_events: bool,

    pub normal_min_volts: f64,
    pub normal_max_volts: f64,
    pub emerg_min_volts: f64,
    pub emerg_max_volts: f64,

    /// `LegalVoltageBases` in kV (no 0.0 terminator; the Vec length rules).
    pub legal_voltage_bases: Vec<f64>,

    /// Scratch for `ProcessBusDefs`/`AddBus` (`NodeBuffer`).
    node_buffer: Vec<i32>,
}

impl Circuit {
    /// Pascal `TDSSCircuit.Create` (Phase 3 fields).
    pub fn new(name: &str, default_base_freq: f64) -> Self {
        Self {
            name: name.to_lowercase(),
            case_name: name.to_string(),
            bus_list: HashList::new(),
            buses: Vec::new(),
            num_nodes: 0,
            map_node_to_bus: vec![NodeBus::default()],
            device_list: HashList::new(),
            num_devices: 0,
            ckt_elements: Vec::new(),
            pd_elements: Vec::new(),
            pc_elements: Vec::new(),
            sources: Vec::new(),
            lines: Vec::new(),
            loads: Vec::new(),
            transformers: Vec::new(),
            solution: Solution::new(default_base_freq),
            fundamental: default_base_freq,
            is_solved: false,
            // Pascal ctor: `BusNameRedefined := TRUE` — forces the first
            // build to create the bus/node lists (SystemYChanged starts true
            // in the Solution ctor, matching the setter's side effect).
            bus_name_redefined: true,
            solution_was_attempted: false,
            load_multiplier: 1.0,
            gen_multiplier: 1.0,
            default_growth_rate: 1.025,
            default_growth_factor: 1.0,
            positive_sequence: false,
            neglect_load_y: false,
            long_line_correction: false,
            duplicates_allowed: false,
            zones_locked: false,
            meter_zones_computed: false,
            log_events: false,
            normal_min_volts: 0.95,
            normal_max_volts: 1.05,
            emerg_min_volts: 0.90,
            emerg_max_volts: 1.08,
            legal_voltage_bases: vec![0.208, 0.480, 12.47, 24.9, 34.5, 115.0, 230.0],
            node_buffer: vec![0; 50],
        }
    }

    /// Pascal `AddCktElement`: register a created element in the device list
    /// and the kind lists, and hand it its 1-based handle.
    pub fn add_ckt_element(&mut self, r: ElemRef, kind: ElemKind, elem: &mut dyn CktElement) {
        self.num_devices += 1;
        self.device_list.add(elem.cd().obj.name());
        self.ckt_elements.push(r);

        match kind {
            ElemKind::Source => self.sources.push(r), // NON_PCPD: sources only
            ElemKind::Line => {
                self.pd_elements.push(r);
                self.lines.push(r);
            }
            ElemKind::Load => {
                self.pc_elements.push(r);
                self.loads.push(r);
            }
            ElemKind::Transformer => {
                self.pd_elements.push(r);
                self.transformers.push(r);
            }
        }
        elem.cd_mut().handle = self.ckt_elements.len();
    }

    /// Pascal `AddBus`: find-or-create the bus, then allocate global node
    /// references for `node_buffer[..n_nodes]`, replacing the user node
    /// numbers in the buffer with the global references ("Caution: Magic").
    /// Returns the bus index (0-based; Pascal returned 1-based).
    fn add_bus(&mut self, bus_name: &str, n_nodes: usize) -> Option<usize> {
        if bus_name.is_empty() {
            // Pascal error 424: null busname; zero the buffer and bail.
            for v in self.node_buffer.iter_mut().take(n_nodes) {
                *v = 0;
            }
            return None;
        }

        let bus_idx = match self.bus_list.find(bus_name) {
            Some(i) => i,
            None => {
                let i = self.bus_list.add(bus_name);
                self.buses.push(Bus::new(bus_name));
                debug_assert_eq!(self.buses.len() - 1, i);
                i
            }
        };

        for k in 0..n_nodes {
            let node_num = self.node_buffer[k];
            // Pascal TDSSBus.Add: node 0 is ground; otherwise find-or-append.
            let node_ref = if node_num == 0 {
                0
            } else {
                let bus = &mut self.buses[bus_idx];
                match bus.find_idx(node_num) {
                    Some(i) => bus.ref_no[i],
                    None => {
                        self.num_nodes += 1;
                        bus.nodes.push(node_num);
                        bus.ref_no.push(self.num_nodes);
                        self.map_node_to_bus.push(NodeBus {
                            bus_ref: bus_idx,
                            node_num,
                        });
                        self.num_nodes
                    }
                }
            };
            self.node_buffer[k] = node_ref as i32;
        }
        Some(bus_idx)
    }

    /// Pascal `TDSSCircuit.ProcessBusDefs(element)`: decode each terminal's
    /// bus spec, allocate buses/nodes, and set the element's node references.
    pub fn process_bus_defs(
        &mut self,
        elem: &mut dyn CktElement,
        parser: &mut Parser,
        vars: &ParserVars,
        errors: &mut Vec<String>,
    ) {
        let np = elem.cd().nphases;
        let ncond = elem.cd().nconds;
        let nterms = elem.cd().nterms;
        if self.node_buffer.len() < ncond + 1 {
            self.node_buffer.resize(ncond + 1, 0);
        }

        for iterm in 1..=nterms {
            let current_bus = elem.cd().get_bus(iterm).to_string();

            // Assume normal phase rotation for the defaults; conductors
            // beyond nphases default to ground.
            for i in 0..ncond {
                self.node_buffer[i] = if i < np { i as i32 + 1 } else { 0 };
            }

            // Parser overrides the defaults with any explicit ".n" list.
            let (bus_name, node_nums) = match parser.parse_as_bus_name(&current_bus, vars) {
                Ok(v) => v,
                Err(e) => {
                    errors.push(e.message().to_string());
                    continue;
                }
            };
            for (i, &n) in node_nums.iter().enumerate().take(self.node_buffer.len()) {
                self.node_buffer[i] = n;
            }

            // Check for error in node specification (negative => bad spec).
            let nodes_ok = !node_nums.iter().any(|&n| n < 0);
            if !nodes_ok {
                errors.push(format!(
                    "Error in Node specification for Element: \"{}\"; Bus Spec: \"{}\"",
                    elem.cd().obj.name(),
                    current_bus
                ));
                continue;
            }

            // AddBus replaces node_buffer values with global references.
            if let Some(bus_idx) = self.add_bus(&bus_name, ncond) {
                elem.cd_mut().terminals[iterm - 1].bus_ref = bus_idx;
                let refs: Vec<usize> = self.node_buffer[..ncond]
                    .iter()
                    .map(|&n| n.max(0) as usize)
                    .collect();
                elem.cd_mut().set_node_ref(iterm, &refs);
            } else {
                errors.push(format!(
                    "TDSSCircuit.AddBus: BusName for Object \"{}\" is null. Error in definition of object.",
                    elem.cd().obj.name()
                ));
            }
        }
        // The per-element call leaves BusNameRedefined handling to the caller.
    }

    /// Pascal `ReprocessBusDefs`: rebuild the bus list and all node
    /// references from scratch, then restore saved per-bus info (kV bases,
    /// coordinates, prior voltages).
    pub fn reprocess_bus_defs(
        &mut self,
        store: &mut dyn ElemStore,
        parser: &mut Parser,
        vars: &ParserVars,
        errors: &mut Vec<String>,
    ) {
        // > SaveBusInfo
        let saved_buses = std::mem::take(&mut self.buses);
        // < (names live inside the saved buses)

        self.bus_list = HashList::new();
        self.num_nodes = 0;
        self.map_node_to_bus = vec![NodeBus::default()];

        // Now redo all enabled circuit elements.
        let refs: Vec<ElemRef> = self.ckt_elements.clone();
        for r in refs {
            let elem = store.ckt_elem_mut(r);
            if elem.cd().enabled {
                self.process_bus_defs(elem, parser, vars, errors);
            }
        }

        for bus in &mut self.buses {
            bus.allocate_bus_state();
        }

        // > RestoreBusInfo
        for saved in &saved_buses {
            let Some(idx) = self.bus_list.find(&saved.name) else {
                continue;
            };
            let bus = &mut self.buses[idx];
            bus.kv_base = saved.kv_base;
            bus.x = saved.x;
            bus.y = saved.y;
            bus.coord_defined = saved.coord_defined;
            bus.keep = saved.keep;
            for (j, &num) in saved.nodes.iter().enumerate() {
                if let Some(jdx) = bus.find_idx(num)
                    && j < saved.vbus.len()
                    && jdx < bus.vbus.len()
                {
                    bus.vbus[jdx] = saved.vbus[j];
                }
            }
        }
        // < RestoreBusInfo

        self.bus_name_redefined = false;
    }

    /// Pascal `Set_BusNameRedefined`: raising the flag also forces a Y
    /// rebuild on the next solution.
    pub fn set_bus_name_redefined(&mut self, value: bool) {
        self.bus_name_redefined = value;
        if value {
            self.solution.system_y_changed = true;
        }
    }

    /// The Y-order node name for global node `i` (1-based), as dss-python's
    /// `YNodeOrder` reports it: `BUSNAME.N` uppercased.
    pub fn node_name(&self, i: usize) -> String {
        let nb = self.map_node_to_bus[i];
        format!(
            "{}.{}",
            self.buses[nb.bus_ref].name.to_uppercase(),
            nb.node_num
        )
    }

    /// Total circuit losses (Pascal `Get_Losses`): sum over enabled PD
    /// elements.
    pub fn losses(
        &mut self,
        store: &mut dyn ElemStore,
        sys: &crate::elements::traits::SysCtx,
    ) -> Complex64 {
        let mut total = Complex64::ZERO;
        let node_v = self.solution.node_v.clone();
        for &r in &self.pd_elements {
            let elem = store.ckt_elem_mut(r);
            if elem.cd().enabled {
                total += elem.losses(sys, &node_v);
            }
        }
        total
    }
}
