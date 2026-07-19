//! Channel-2 protocol tests (WASM_USERMODELS_PLAN §2.5): the whole host
//! protocol driven against tiny inline-WAT guests — hermetic, per-commit,
//! zero external tooling. Covers the WP-WM.1 prescription: happy-path
//! 15-function round trip, record byte-exact round trip, each callback tier
//! (parser callbacks against the owned AuxParser, `control_queue_push`
//! queuing), missing export → the exact 569-path name, and the loud typed
//! errors for trap / fuel exhaustion / OOB / memory-cap.

use dss_usermodel::{
    Callbacks, CapControlInstance, DynamicsRec, Effect, GeneratorVars, HostConfig, InterfaceKind,
    NoCallbacks, Shuttle, UserModelError, UserModelHost, UserModelInstance,
};
use num_complex::Complex64;

// ---------------------------------------------------------------------------
// WAT guest templates
// ---------------------------------------------------------------------------

/// The full 15-function Generator-form guest. Slot area: 64 f64 slots at
/// 1024 (1-based; slot k at 1024+(k-1)*8). Slots 1..11 count calls (binding
/// order: new select init calc integrate save restore edit update_model
/// delete num_vars); 16 = edit len, 17 = edit byte checksum, 18 = last
/// `select` argument, 20 = `dynarec.h` seen by `calc`. `integrate` mutates
/// the shared records: `genvars.Pshaft += 1.0`, `dynarec.t += dynarec.h`.
/// `{imports}`, `{data}` and `{calc_extra}` specialize the guest per test.
fn guest_15(imports: &str, data: &str, calc_extra: &str) -> Vec<u8> {
    let wat = format!(
        r#"(module
  {imports}
  (memory (export "memory") 2)
  {data}
  (global $heap (mut i32) (i32.const 8192))
  (global $genvars (mut i32) (i32.const 0))
  (global $dynarec (mut i32) (i32.const 0))
  (func $slot_addr (param $k i32) (result i32)
    (i32.add (i32.const 1024)
             (i32.mul (i32.sub (local.get $k) (i32.const 1)) (i32.const 8))))
  (func $bump (param $k i32)
    (f64.store (call $slot_addr (local.get $k))
      (f64.add (f64.load (call $slot_addr (local.get $k))) (f64.const 1))))
  (func $set_slot (param $k i32) (param $v f64)
    (f64.store (call $slot_addr (local.get $k)) (local.get $v)))
  (func $cksum (param $ptr i32) (result i32)
    (local $sum i32)
    (block $done
      (loop $loop
        (br_if $done (i32.eqz (i32.load8_u (local.get $ptr))))
        (local.set $sum (i32.add (local.get $sum) (i32.load8_u (local.get $ptr))))
        (local.set $ptr (i32.add (local.get $ptr) (i32.const 1)))
        (br $loop)))
    (local.get $sum))
  (func (export "dss_alloc") (param $size i32) (result i32)
    (local $ptr i32)
    (local.set $ptr (global.get $heap))
    (global.set $heap
      (i32.and (i32.add (i32.add (global.get $heap) (local.get $size)) (i32.const 7))
               (i32.const -8)))
    (local.get $ptr))
  (func (export "new") (param $gv i32) (param $dr i32) (result i32)
    (global.set $genvars (local.get $gv))
    (global.set $dynarec (local.get $dr))
    (call $bump (i32.const 1))
    (i32.const 7))
  (func (export "select") (param $id i32) (result i32)
    (call $bump (i32.const 2))
    (call $set_slot (i32.const 18) (f64.convert_i32_s (local.get $id)))
    (local.get $id))
  (func (export "init") (param $v i32) (param $i i32)
    (call $bump (i32.const 3)))
  (func (export "calc") (param $v i32) (param $i i32)
    (call $bump (i32.const 4))
    (call $set_slot (i32.const 20) (f64.load (global.get $dynarec)))
    (f64.store (local.get $i)
      (f64.mul (f64.load (local.get $v)) (f64.const 2)))
    (f64.store (i32.add (local.get $i) (i32.const 8))
      (f64.sub (f64.load (i32.add (local.get $v) (i32.const 8))) (f64.const 1)))
    {calc_extra})
  (func (export "integrate")
    (call $bump (i32.const 5))
    (f64.store (i32.add (global.get $genvars) (i32.const 8))
      (f64.add (f64.load (i32.add (global.get $genvars) (i32.const 8))) (f64.const 1)))
    (f64.store (i32.add (global.get $dynarec) (i32.const 8))
      (f64.add (f64.load (i32.add (global.get $dynarec) (i32.const 8)))
               (f64.load (global.get $dynarec)))))
  (func (export "save") (call $bump (i32.const 6)))
  (func (export "restore") (call $bump (i32.const 7)))
  (func (export "edit") (param $ptr i32) (param $len i32)
    (local $k i32) (local $sum i32)
    (call $bump (i32.const 8))
    (call $set_slot (i32.const 16) (f64.convert_i32_s (local.get $len)))
    (block $done
      (loop $loop
        (br_if $done (i32.ge_u (local.get $k) (local.get $len)))
        (local.set $sum
          (i32.add (local.get $sum) (i32.load8_u (i32.add (local.get $ptr) (local.get $k)))))
        (local.set $k (i32.add (local.get $k) (i32.const 1)))
        (br $loop)))
    (call $set_slot (i32.const 17) (f64.convert_i32_s (local.get $sum))))
  (func (export "update_model") (call $bump (i32.const 9)))
  (func (export "delete") (param $id i32) (call $bump (i32.const 10)))
  (func (export "num_vars") (result i32) (call $bump (i32.const 11)) (i32.const 64))
  (func (export "get_all_vars") (param $ptr i32)
    (local $k i32)
    (block $done
      (loop $loop
        (br_if $done (i32.ge_u (local.get $k) (i32.const 64)))
        (f64.store (i32.add (local.get $ptr) (i32.mul (local.get $k) (i32.const 8)))
          (f64.load (i32.add (i32.const 1024) (i32.mul (local.get $k) (i32.const 8)))))
        (local.set $k (i32.add (local.get $k) (i32.const 1)))
        (br $loop))))
  (func (export "get_variable") (param $i i32) (result f64)
    (f64.load (call $slot_addr (local.get $i))))
  (func (export "set_variable") (param $i i32) (param $v f64)
    (call $set_slot (local.get $i) (local.get $v)))
  (func (export "get_var_name") (param $i i32) (param $ptr i32) (param $maxlen i32)
    (i32.store8 (local.get $ptr) (i32.const 117))
    (i32.store8 (i32.add (local.get $ptr) (i32.const 1)) (i32.const 118))
    (i32.store8 (i32.add (local.get $ptr) (i32.const 2)) (i32.const 97))
    (i32.store8 (i32.add (local.get $ptr) (i32.const 3)) (i32.const 114))
    (i32.store8 (i32.add (local.get $ptr) (i32.const 4)) (i32.const 0)))
)"#
    );
    wat::parse_str(&wat).expect("template WAT must assemble")
}

/// A minimal stub module: memory + `dss_alloc` + the Generator-form 15
/// functions, with `skip`ped exports omitted and `override_defs` replacing
/// whole definitions by export name (for signature-mismatch tests).
fn stub_15_without(skip: &[&str], override_defs: &[(&str, &str)]) -> Vec<u8> {
    let defs: &[(&str, &str)] = &[
        (
            "dss_alloc",
            r#"(func (export "dss_alloc") (param i32) (result i32) (i32.const 64))"#,
        ),
        (
            "new",
            r#"(func (export "new") (param i32 i32) (result i32) (i32.const 1))"#,
        ),
        (
            "select",
            r#"(func (export "select") (param i32) (result i32) (i32.const 1))"#,
        ),
        ("init", r#"(func (export "init") (param i32 i32))"#),
        ("calc", r#"(func (export "calc") (param i32 i32))"#),
        ("integrate", r#"(func (export "integrate"))"#),
        ("save", r#"(func (export "save"))"#),
        ("restore", r#"(func (export "restore"))"#),
        ("edit", r#"(func (export "edit") (param i32 i32))"#),
        ("update_model", r#"(func (export "update_model"))"#),
        ("delete", r#"(func (export "delete") (param i32))"#),
        (
            "num_vars",
            r#"(func (export "num_vars") (result i32) (i32.const 0))"#,
        ),
        (
            "get_all_vars",
            r#"(func (export "get_all_vars") (param i32))"#,
        ),
        (
            "get_variable",
            r#"(func (export "get_variable") (param i32) (result f64) (f64.const 0))"#,
        ),
        (
            "set_variable",
            r#"(func (export "set_variable") (param i32 f64))"#,
        ),
        (
            "get_var_name",
            r#"(func (export "get_var_name") (param i32 i32 i32))"#,
        ),
    ];
    let mut wat = String::from("(module (memory (export \"memory\") 1)\n");
    for (name, def) in defs {
        if skip.contains(name) {
            continue;
        }
        let def = override_defs
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, d)| *d)
            .unwrap_or(def);
        wat.push_str(def);
        wat.push('\n');
    }
    wat.push(')');
    wat::parse_str(&wat).expect("stub WAT must assemble")
}

/// Distinct-valued records for shuttle tests.
fn distinct_gen_vars() -> GeneratorVars {
    GeneratorVars {
        theta: 1.0,
        pshaft: 10.0,
        speed: 3.0,
        w0: 376.9911,
        hmass: 5.0,
        mmass: 6.0,
        d: 7.0,
        dpu: 8.0,
        kva_rating: 1000.0,
        kv_generator_base: 12.47,
        xd: 11.0,
        xdp: 12.0,
        xdpp: 13.0,
        pu_xd: 14.0,
        pu_xdp: 15.0,
        pu_xdpp: 16.0,
        dtheta: 17.0,
        dspeed: 18.0,
        theta_history: 19.0,
        speed_history: 20.0,
        pnominalperphase: 21.0,
        qnominalperphase: 22.0,
        num_phases: 3,
        num_conductors: 4,
        conn: 1,
        vthev_mag: 26.0,
        vthev_harm: 27.0,
        theta_harm: 28.0,
        vtarget: 29.0,
        zthev: (30.0, 31.0),
        xrdp: 32.0,
    }
}

fn distinct_dyn_rec() -> DynamicsRec {
    DynamicsRec {
        h: 0.25,
        t: 1.0,
        tstart: 0.0,
        tstop: 5.0,
        iteration_flag: 1,
        solution_mode: 14, // DYNAMICMODE
        int_hour: 2,
        dbl_hour: 3.5,
    }
}

/// Shuttle with a fresh `NoCallbacks` context.
macro_rules! sh {
    ($g:expr, $d:expr) => {
        Shuttle {
            gen_vars: Some($g),
            dyn_rec: $d,
            ctx: Box::new(NoCallbacks),
        }
    };
}

// ---------------------------------------------------------------------------
// Happy path: the 15-function round trip + record shuttle
// ---------------------------------------------------------------------------

#[test]
fn happy_path_15_function_round_trip() {
    let bytes = guest_15("", "", "");
    let host = UserModelHost::load(
        "proto15.wasm",
        &bytes,
        InterfaceKind::GenUserModel,
        HostConfig::default(),
    )
    .expect("load");

    let mut gvars = distinct_gen_vars();
    let mut dyn_rec = distinct_dyn_rec();

    let mut inst =
        UserModelInstance::new(&host, 2, sh!(&mut gvars, &mut dyn_rec)).expect("instance");
    assert_eq!(inst.id(), 7);
    assert!(inst.exists());
    assert_eq!(inst.kind(), InterfaceKind::GenUserModel);
    assert_eq!(inst.model(), "proto15.wasm");

    // select: returns the id, records it in slot 18.
    let sel = inst.select(sh!(&mut gvars, &mut dyn_rec)).expect("select");
    assert_eq!(sel, 7);

    // edit: the string reaches the guest byte-for-byte (len + checksum).
    let user_data = "kw=3.2 kvar=1";
    inst.edit(user_data, sh!(&mut gvars, &mut dyn_rec))
        .expect("edit");

    // init + calc: V in, I out; the guest sees the dynarec image (slot 20).
    let v = [Complex64::new(3.0, 4.0), Complex64::new(5.0, 6.0)];
    let mut i = [Complex64::new(9.0, 9.0), Complex64::new(7.0, 7.0)];
    inst.init(&v, &mut i, sh!(&mut gvars, &mut dyn_rec))
        .expect("init");
    let mut i = [Complex64::new(9.0, 9.0), Complex64::new(7.0, 7.0)];
    inst.calc(&v, &mut i, sh!(&mut gvars, &mut dyn_rec))
        .expect("calc");
    assert_eq!(i[0], Complex64::new(6.0, 3.0)); // guest: (2*re, im-1)
    assert_eq!(i[1], Complex64::new(7.0, 7.0)); // untouched entry round-trips

    // integrate mutates the shared records (checked in detail below).
    inst.integrate(sh!(&mut gvars, &mut dyn_rec))
        .expect("integrate");
    assert_eq!(gvars.pshaft, 11.0);
    assert_eq!(dyn_rec.t, 1.25);

    inst.save(sh!(&mut gvars, &mut dyn_rec)).expect("save");
    inst.restore(sh!(&mut gvars, &mut dyn_rec))
        .expect("restore");
    inst.update_model(sh!(&mut gvars, &mut dyn_rec))
        .expect("update_model");

    // The monitoring surface.
    let n = inst
        .num_vars(sh!(&mut gvars, &mut dyn_rec))
        .expect("num_vars");
    assert_eq!(n, 64);
    inst.set_variable(19, 42.5, sh!(&mut gvars, &mut dyn_rec))
        .expect("set_variable");
    let v19 = inst
        .get_variable(19, sh!(&mut gvars, &mut dyn_rec))
        .expect("get_variable");
    assert_eq!(v19, 42.5);
    let edit_len = inst
        .get_variable(16, sh!(&mut gvars, &mut dyn_rec))
        .expect("get_variable");
    assert_eq!(edit_len, user_data.len() as f64);
    let edit_sum = inst
        .get_variable(17, sh!(&mut gvars, &mut dyn_rec))
        .expect("get_variable");
    assert_eq!(
        edit_sum,
        user_data.bytes().map(u32::from).sum::<u32>() as f64
    );
    let name = inst
        .get_var_name(1, sh!(&mut gvars, &mut dyn_rec))
        .expect("get_var_name");
    assert_eq!(name, "uvar");

    // get_all_vars: read the call counters back (binding-order slots).
    let mut all = [0f64; 64];
    inst.get_all_vars(&mut all, sh!(&mut gvars, &mut dyn_rec))
        .expect("get_all_vars");
    assert_eq!(all[0], 1.0, "new called once");
    assert_eq!(all[1], 2.0, "select: once explicit + once inside integrate");
    assert_eq!(all[2], 1.0, "init");
    assert_eq!(all[3], 1.0, "calc");
    assert_eq!(all[4], 1.0, "integrate");
    assert_eq!(all[5], 1.0, "save");
    assert_eq!(all[6], 1.0, "restore");
    assert_eq!(all[7], 1.0, "edit");
    assert_eq!(all[8], 1.0, "update_model");
    assert_eq!(all[9], 0.0, "delete not yet called");
    assert_eq!(all[10], 1.0, "num_vars");
    assert_eq!(all[17], 7.0, "slot 18: last select argument = id");
    assert_eq!(all[19], 0.25, "slot 20: dynarec.h seen by calc");

    // delete clears the id (Pascal Destroy path).
    inst.delete(sh!(&mut gvars, &mut dyn_rec)).expect("delete");
    assert!(!inst.exists());
    let del_count = inst
        .get_variable(10, sh!(&mut gvars, &mut dyn_rec))
        .expect("get_variable");
    assert_eq!(del_count, 1.0, "guest delete ran once");

    // No effects were queued by this guest.
    assert!(inst.drain_effects().is_empty());
}

/// Record shuttle byte-exactness: write image → guest increments a field →
/// read back; every untouched byte identical, mutated fields exact.
#[test]
fn record_shuttle_byte_exact_round_trip() {
    let bytes = guest_15("", "", "");
    let host = UserModelHost::load(
        "shuttle.wasm",
        &bytes,
        InterfaceKind::GenUserModel,
        HostConfig::default(),
    )
    .expect("load");

    let mut gvars = distinct_gen_vars();
    let mut dyn_rec = distinct_dyn_rec();
    let gvars0 = gvars;
    let dyn0 = dyn_rec;

    let mut inst =
        UserModelInstance::new(&host, 1, sh!(&mut gvars, &mut dyn_rec)).expect("instance");
    // `new` must round-trip both records byte-exactly (the guest does not
    // touch them there).
    assert_eq!(gvars.to_bytes(), gvars0.to_bytes());
    assert_eq!(dyn_rec.to_bytes(), dyn0.to_bytes());

    inst.integrate(sh!(&mut gvars, &mut dyn_rec))
        .expect("integrate");

    // Expected: exactly Pshaft += 1.0 and t += h; everything else identical.
    let mut gen_want = gvars0;
    gen_want.pshaft += 1.0;
    let mut dyn_want = dyn0;
    dyn_want.t += dyn_want.h;
    assert_eq!(gvars.to_bytes(), gen_want.to_bytes());
    assert_eq!(dyn_rec.to_bytes(), dyn_want.to_bytes());
}

// ---------------------------------------------------------------------------
// Callback tier A: pure reads from the context snapshot
// ---------------------------------------------------------------------------

/// Full-surface tier-A snapshot with recognizable values.
struct TierACtx {
    voltages: Vec<Complex64>,
    currents: Vec<Complex64>,
    node_v: Vec<Complex64>,
    node_refs: Vec<i32>,
    public: Vec<u8>,
}

impl TierACtx {
    fn new() -> Self {
        let public_gen = GeneratorVars {
            pshaft: 777.5,
            ..Default::default()
        };
        Self {
            voltages: vec![
                Complex64::new(7.0, 1.0),
                Complex64::new(8.0, 2.0),
                Complex64::new(9.0, 3.0),
            ],
            currents: vec![Complex64::new(0.5, -0.5), Complex64::new(1.5, -1.5)],
            node_v: vec![
                Complex64::new(1.0, 10.0),
                Complex64::new(2.0, 20.0),
                Complex64::new(3.0, 30.0),
                Complex64::new(4.0, 40.0),
            ],
            node_refs: vec![11, 12, 13],
            public: public_gen.to_bytes().to_vec(),
        }
    }
}

impl Callbacks for TierACtx {
    fn active_element_name(&self) -> Option<&str> {
        Some("Generator.g1")
    }
    fn active_element_index(&self) -> i32 {
        42
    }
    fn active_element_bus_names(&self) -> (&str, &str) {
        ("busA", "busB")
    }
    fn active_element_voltages(&self) -> Option<&[Complex64]> {
        Some(&self.voltages)
    }
    fn active_element_currents(&self) -> Option<&[Complex64]> {
        Some(&self.currents)
    }
    fn active_element_losses(&self) -> [Complex64; 3] {
        [
            Complex64::new(100.0, 1.0),
            Complex64::new(50.0, 2.0),
            Complex64::new(25.0, 3.0),
        ]
    }
    fn active_element_power(&self, terminal: i32) -> Complex64 {
        Complex64::new(f64::from(terminal) * 10.0, -f64::from(terminal))
    }
    fn active_element_num_cust(&self) -> (i32, i32) {
        (4, 9)
    }
    fn active_element_node_refs(&self) -> Option<&[i32]> {
        Some(&self.node_refs)
    }
    fn active_element_bus_ref(&self, terminal: i32) -> i32 {
        20 + terminal
    }
    fn active_element_terminal_info(&self) -> Option<(i32, i32, i32)> {
        Some((2, 4, 3))
    }
    fn is_active_element_enabled(&self) -> bool {
        true
    }
    fn node_voltages(&self) -> &[Complex64] {
        &self.node_v
    }
    fn is_bus_coordinate_defined(&self, bus_ref: i32) -> bool {
        bus_ref == 3
    }
    fn bus_coordinate(&self, bus_ref: i32) -> (f64, f64) {
        if bus_ref == 3 {
            (150.5, -60.25)
        } else {
            (0.0, 0.0)
        }
    }
    fn bus_kv_base(&self, bus_ref: i32) -> f64 {
        if bus_ref == 5 { 13.8 } else { 0.0 }
    }
    fn bus_dist_from_meter(&self, bus_ref: i32) -> f64 {
        if bus_ref == 4 { 2.5 } else { 0.0 }
    }
    fn dynamics_rec(&self) -> Option<DynamicsRec> {
        Some(DynamicsRec {
            h: 0.125,
            solution_mode: 14,
            ..Default::default()
        })
    }
    fn step_size(&self) -> f64 {
        0.05
    }
    fn time_sec(&self) -> f64 {
        12.5
    }
    fn time_hr(&self) -> f64 {
        1.75
    }
    fn public_data(&self) -> &[u8] {
        &self.public
    }
}

const TIER_A_IMPORTS: &str = r#"
  (import "dss_env" "get_step_size" (func $get_step_size (result f64)))
  (import "dss_env" "get_time_sec" (func $get_time_sec (result f64)))
  (import "dss_env" "get_time_hr" (func $get_time_hr (result f64)))
  (import "dss_env" "get_bus_kv_base" (func $get_bus_kv_base (param i32) (result f64)))
  (import "dss_env" "get_bus_dist_from_meter" (func $get_bus_dist (param i32) (result f64)))
  (import "dss_env" "get_active_element_bus_ref" (func $get_bus_ref (param i32) (result i32)))
  (import "dss_env" "is_active_element_enabled" (func $is_enabled (result i32)))
  (import "dss_env" "is_bus_coordinate_defined" (func $is_coord_def (param i32) (result i32)))
  (import "dss_env" "get_active_element_index" (func $get_index (result i32)))
  (import "dss_env" "get_active_element_voltages" (func $get_voltages (param i32 i32)))
  (import "dss_env" "get_active_element_currents" (func $get_currents (param i32 i32)))
  (import "dss_env" "get_active_element_losses" (func $get_losses (param i32 i32 i32)))
  (import "dss_env" "get_active_element_power" (func $get_power (param i32 i32)))
  (import "dss_env" "get_active_element_num_cust" (func $get_num_cust (param i32 i32)))
  (import "dss_env" "get_active_element_node_ref" (func $get_node_ref (param i32 i32)))
  (import "dss_env" "get_active_element_terminal_info" (func $get_term_info (param i32 i32 i32)))
  (import "dss_env" "get_node_voltages" (func $get_node_v (param i32 i32) (result i32)))
  (import "dss_env" "get_dynamics_rec" (func $get_dyn (param i32)))
  (import "dss_env" "get_public_data" (func $get_public (param i32 i32) (result i32)))
  (import "dss_env" "get_active_element_name" (func $get_name (param i32 i32) (result i32)))
  (import "dss_env" "get_bus_coordinate" (func $get_coord (param i32 i32 i32)))
  (import "dss_env" "get_active_element_bus_names" (func $get_bus_names (param i32 i32 i32 i32)))
"#;

/// `calc` body exercising every tier-A import into slots 21..54.
const TIER_A_CALC: &str = r#"
    (call $set_slot (i32.const 21) (call $get_step_size))
    (call $set_slot (i32.const 22) (call $get_time_sec))
    (call $set_slot (i32.const 23) (call $get_time_hr))
    (call $set_slot (i32.const 24) (call $get_bus_kv_base (i32.const 5)))
    (call $set_slot (i32.const 25) (f64.convert_i32_s (call $get_bus_ref (i32.const 2))))
    (call $set_slot (i32.const 26) (f64.convert_i32_s (call $is_enabled)))
    (call $set_slot (i32.const 27) (f64.convert_i32_s (call $get_index)))
    ;; voltages: buffer size 4 at num=1552, V buffer at 1600
    (i32.store (i32.const 1552) (i32.const 4))
    (call $get_voltages (i32.const 1552) (i32.const 1600))
    (call $set_slot (i32.const 28) (f64.convert_i32_s (i32.load (i32.const 1552))))
    (call $set_slot (i32.const 29) (f64.load (i32.const 1616))) ;; V[2].re
    ;; dynamics record copy at 1792
    (call $get_dyn (i32.const 1792))
    (call $set_slot (i32.const 30) (f64.load (i32.const 1792)))
    (call $set_slot (i32.const 31)
      (f64.convert_i32_s (i32.load (i32.const 1828)))) ;; solution_mode at +36
    ;; public data at 2048
    (call $set_slot (i32.const 32)
      (f64.convert_i32_s (call $get_public (i32.const 2048) (i32.const 244))))
    (call $set_slot (i32.const 33) (f64.load (i32.const 2056))) ;; Pshaft at +8
    ;; node voltages, max 3, at 2560
    (call $set_slot (i32.const 34)
      (f64.convert_i32_s (call $get_node_v (i32.const 2560) (i32.const 3))))
    (call $set_slot (i32.const 35) (f64.load (i32.const 2576))) ;; node 2 re
    ;; element name at 2700, maxlen 31
    (call $set_slot (i32.const 36)
      (f64.convert_i32_s (call $get_name (i32.const 2700) (i32.const 31))))
    (call $set_slot (i32.const 37) (f64.convert_i32_s (call $cksum (i32.const 2700))))
    ;; terminal info
    (call $get_term_info (i32.const 2940) (i32.const 2944) (i32.const 2948))
    (call $set_slot (i32.const 38) (f64.convert_i32_s (i32.load (i32.const 2940))))
    (call $set_slot (i32.const 39) (f64.convert_i32_s (i32.load (i32.const 2944))))
    (call $set_slot (i32.const 40) (f64.convert_i32_s (i32.load (i32.const 2948))))
    ;; bus coordinate of bus 3
    (call $get_coord (i32.const 3) (i32.const 2960) (i32.const 2968))
    (call $set_slot (i32.const 41) (f64.load (i32.const 2960)))
    (call $set_slot (i32.const 42) (f64.load (i32.const 2968)))
    ;; losses (total 3000, load 3016, noload 3032)
    (call $get_losses (i32.const 3000) (i32.const 3016) (i32.const 3032))
    (call $set_slot (i32.const 43) (f64.load (i32.const 3016)))
    ;; power at terminal 2 (3072)
    (call $get_power (i32.const 2) (i32.const 3072))
    (call $set_slot (i32.const 44) (f64.load (i32.const 3072)))
    (call $set_slot (i32.const 45) (f64.load (i32.const 3080)))
    (call $set_slot (i32.const 46) (f64.convert_i32_s (call $is_coord_def (i32.const 3))))
    (call $set_slot (i32.const 47) (call $get_bus_dist (i32.const 4)))
    ;; num cust (3120, 3124)
    (call $get_num_cust (i32.const 3120) (i32.const 3124))
    (call $set_slot (i32.const 48) (f64.convert_i32_s (i32.load (i32.const 3120))))
    (call $set_slot (i32.const 49) (f64.convert_i32_s (i32.load (i32.const 3124))))
    ;; currents: buffer size 2 at 3140, buffer at 3200
    (i32.store (i32.const 3140) (i32.const 2))
    (call $get_currents (i32.const 3140) (i32.const 3200))
    (call $set_slot (i32.const 50) (f64.convert_i32_s (i32.load (i32.const 3140))))
    (call $set_slot (i32.const 51) (f64.load (i32.const 3208))) ;; I[1].im
    ;; node refs, maxsize 4, at 3260
    (call $get_node_ref (i32.const 4) (i32.const 3260))
    (call $set_slot (i32.const 52) (f64.convert_i32_s (i32.load (i32.const 3268)))) ;; ref[3]
    ;; bus names at 3300 / 3340, maxlen 15
    (call $get_bus_names (i32.const 3300) (i32.const 15) (i32.const 3340) (i32.const 15))
    (call $set_slot (i32.const 53) (f64.convert_i32_s (call $cksum (i32.const 3300))))
    (call $set_slot (i32.const 54) (f64.convert_i32_s (call $cksum (i32.const 3340))))
"#;

#[test]
fn tier_a_callbacks_serve_the_context_snapshot() {
    let bytes = guest_15(TIER_A_IMPORTS, "", TIER_A_CALC);
    let host = UserModelHost::load(
        "tier_a.wasm",
        &bytes,
        InterfaceKind::GenUserModel,
        HostConfig::default(),
    )
    .expect("load");

    let mut gvars = distinct_gen_vars();
    let mut dyn_rec = distinct_dyn_rec();
    let mut inst =
        UserModelInstance::new(&host, 1, sh!(&mut gvars, &mut dyn_rec)).expect("instance");

    let v = [Complex64::new(1.0, 0.0)];
    let mut i = [Complex64::new(0.0, 0.0)];
    inst.calc(
        &v,
        &mut i,
        Shuttle {
            gen_vars: Some(&mut gvars),
            dyn_rec: &mut dyn_rec,
            ctx: Box::new(TierACtx::new()),
        },
    )
    .expect("calc with tier-A context");

    let mut all = [0f64; 64];
    inst.get_all_vars(&mut all, sh!(&mut gvars, &mut dyn_rec))
        .expect("get_all_vars");
    let slot = |k: usize| all[k - 1];

    assert_eq!(slot(21), 0.05, "get_step_size");
    assert_eq!(slot(22), 12.5, "get_time_sec");
    assert_eq!(slot(23), 1.75, "get_time_hr");
    assert_eq!(slot(24), 13.8, "get_bus_kv_base(5)");
    assert_eq!(slot(25), 22.0, "get_active_element_bus_ref(2)");
    assert_eq!(slot(26), 1.0, "is_active_element_enabled");
    assert_eq!(slot(27), 42.0, "get_active_element_index");
    assert_eq!(slot(28), 3.0, "voltages count = min(buffer 4, yorder 3)");
    assert_eq!(slot(29), 8.0, "V[2].re");
    assert_eq!(slot(30), 0.125, "dynamics rec h");
    assert_eq!(slot(31), 14.0, "dynamics rec solution_mode");
    assert_eq!(slot(32), 244.0, "public data bytes copied");
    assert_eq!(slot(33), 777.5, "public GeneratorVars.Pshaft at +8");
    assert_eq!(slot(34), 3.0, "node voltages copied = min(max 3, 4)");
    assert_eq!(slot(35), 2.0, "node V[2].re");
    assert_eq!(slot(36), 12.0, "element name length");
    assert_eq!(
        slot(37),
        "Generator.g1".bytes().map(u32::from).sum::<u32>() as f64,
        "element name bytes"
    );
    assert_eq!(
        (slot(38), slot(39), slot(40)),
        (2.0, 4.0, 3.0),
        "terminal info"
    );
    assert_eq!((slot(41), slot(42)), (150.5, -60.25), "bus coordinate");
    assert_eq!(slot(43), 50.0, "load losses re");
    assert_eq!((slot(44), slot(45)), (20.0, -2.0), "power at terminal 2");
    assert_eq!(slot(46), 1.0, "is_bus_coordinate_defined(3)");
    assert_eq!(slot(47), 2.5, "bus dist from meter(4)");
    assert_eq!((slot(48), slot(49)), (4.0, 9.0), "num cust");
    assert_eq!(slot(50), 2.0, "currents count");
    assert_eq!(slot(51), -0.5, "I[1].im");
    assert_eq!(slot(52), 13.0, "node ref[3]");
    assert_eq!(
        slot(53),
        "busA".bytes().map(u32::from).sum::<u32>() as f64,
        "bus name 1"
    );
    assert_eq!(
        slot(54),
        "busB".bytes().map(u32::from).sum::<u32>() as f64,
        "bus name 2"
    );
}

/// With no active element (`NoCallbacks`), the Pascal nil paths hold: the
/// `Exit`-without-touching routines leave guest memory unchanged, the
/// zero-first routines write zeros.
#[test]
fn tier_a_defaults_mirror_pascal_nil_paths() {
    let bytes = guest_15(TIER_A_IMPORTS, "", TIER_A_CALC);
    let host = UserModelHost::load(
        "tier_a_nil.wasm",
        &bytes,
        InterfaceKind::GenUserModel,
        HostConfig::default(),
    )
    .expect("load");

    let mut gvars = distinct_gen_vars();
    let mut dyn_rec = distinct_dyn_rec();
    let mut inst =
        UserModelInstance::new(&host, 1, sh!(&mut gvars, &mut dyn_rec)).expect("instance");
    let v = [Complex64::new(1.0, 0.0)];
    let mut i = [Complex64::new(0.0, 0.0)];
    inst.calc(&v, &mut i, sh!(&mut gvars, &mut dyn_rec))
        .expect("calc with empty context");

    let mut all = [0f64; 64];
    inst.get_all_vars(&mut all, sh!(&mut gvars, &mut dyn_rec))
        .expect("get_all_vars");
    let slot = |k: usize| all[k - 1];
    assert_eq!(slot(21), 0.0, "step size default");
    assert_eq!(slot(26), 0.0, "not enabled");
    assert_eq!(slot(27), 0.0, "no index");
    // GetActiveElementVoltagesCallBack Exit path: the guest's stored buffer
    // size (4) stays untouched.
    assert_eq!(slot(28), 4.0, "voltages num untouched (Pascal Exit)");
    assert_eq!(slot(30), 0.0, "dynamics rec untouched (zero memory)");
    assert_eq!(slot(32), 0.0, "no public data");
    assert_eq!(slot(34), 0.0, "no node voltages");
    assert_eq!(slot(36), 0.0, "no element name, result 0");
    assert_eq!(
        (slot(38), slot(39), slot(40)),
        (0.0, 0.0, 0.0),
        "terminal info untouched"
    );
    assert_eq!(slot(46), 0.0, "no coordinate");
    assert_eq!((slot(48), slot(49)), (0.0, 0.0), "num cust zeroed");
    assert_eq!(slot(53), 0.0, "empty bus name 1");
}

// ---------------------------------------------------------------------------
// Callback tier B: effect queue (msg + control queue push)
// ---------------------------------------------------------------------------

struct TierBCtx;

impl Callbacks for TierBCtx {
    fn control_queue_next_handle(&self) -> i32 {
        100
    }
}

#[test]
fn tier_b_effects_queue_in_order_with_sequential_handles() {
    let imports = r#"
  (import "dss_env" "msg_callback" (func $msg (param i32 i32)))
  (import "dss_env" "control_queue_push" (func $push (param i32 f64 i32 i32) (result i32)))
"#;
    let data = r#"(data (i32.const 400) "hello from guest")"#;
    let calc_extra = r#"
    (call $msg (i32.const 400) (i32.const 16))
    (call $set_slot (i32.const 21)
      (f64.convert_i32_s (call $push (i32.const 12) (f64.const 30.5) (i32.const 42) (i32.const 7))))
    (call $set_slot (i32.const 22)
      (f64.convert_i32_s (call $push (i32.const 13) (f64.const 60.25) (i32.const 43) (i32.const 8))))
"#;
    let bytes = guest_15(imports, data, calc_extra);
    let host = UserModelHost::load(
        "tier_b.wasm",
        &bytes,
        InterfaceKind::GenUserModel,
        HostConfig::default(),
    )
    .expect("load");

    let mut gvars = distinct_gen_vars();
    let mut dyn_rec = distinct_dyn_rec();
    let mut inst =
        UserModelInstance::new(&host, 1, sh!(&mut gvars, &mut dyn_rec)).expect("instance");
    assert!(inst.drain_effects().is_empty(), "new queued nothing");

    let v = [Complex64::new(1.0, 0.0)];
    let mut i = [Complex64::new(0.0, 0.0)];
    inst.calc(
        &v,
        &mut i,
        Shuttle {
            gen_vars: Some(&mut gvars),
            dyn_rec: &mut dyn_rec,
            ctx: Box::new(TierBCtx),
        },
    )
    .expect("calc");

    let effects = inst.drain_effects();
    assert_eq!(
        effects,
        vec![
            Effect::Msg("hello from guest".to_string()),
            Effect::ControlQueuePush {
                hour: 12,
                sec: 30.5,
                code: 42,
                proxy_hdl: 7,
                handle: 100,
            },
            Effect::ControlQueuePush {
                hour: 13,
                sec: 60.25,
                code: 43,
                proxy_hdl: 8,
                handle: 101,
            },
        ],
        "order preserved, provisional handles sequential from the seed"
    );
    assert!(inst.drain_effects().is_empty(), "drain empties the queue");

    // The guest observed the same handles the host recorded.
    let mut all = [0f64; 64];
    inst.get_all_vars(&mut all, sh!(&mut gvars, &mut dyn_rec))
        .expect("get_all_vars");
    assert_eq!(all[20], 100.0);
    assert_eq!(all[21], 101.0);
}

// ---------------------------------------------------------------------------
// Callback tier C: the owned AuxParser
// ---------------------------------------------------------------------------

#[test]
fn tier_c_parser_callbacks_drive_the_owned_aux_parser() {
    let imports = r#"
  (import "dss_env" "load_parser" (func $load_parser (param i32 i32)))
  (import "dss_env" "next_param" (func $next_param (param i32 i32) (result i32)))
  (import "dss_env" "get_dbl_value" (func $get_dbl (param i32)))
  (import "dss_env" "get_int_value" (func $get_int (param i32)))
  (import "dss_env" "get_str_value" (func $get_str (param i32 i32)))
"#;
    let data = r#"(data (i32.const 400) "kw=3.25 n=7 name=fred")"#;
    let calc_extra = r#"
    (call $load_parser (i32.const 400) (i32.const 21))
    ;; param 1: name "kw", value "3.25" (len 4)
    (call $set_slot (i32.const 21)
      (f64.convert_i32_s (call $next_param (i32.const 640) (i32.const 31))))
    (call $set_slot (i32.const 22) (f64.convert_i32_s (call $cksum (i32.const 640))))
    (call $get_dbl (i32.const 672))
    (call $set_slot (i32.const 23) (f64.load (i32.const 672)))
    ;; param 2: name "n", value "7" (len 1)
    (call $set_slot (i32.const 24)
      (f64.convert_i32_s (call $next_param (i32.const 640) (i32.const 31))))
    (call $get_int (i32.const 680))
    (call $set_slot (i32.const 25) (f64.convert_i32_s (i32.load (i32.const 680))))
    ;; param 3: name "name", value "fred" (len 4) via get_str_value
    (call $set_slot (i32.const 26)
      (f64.convert_i32_s (call $next_param (i32.const 640) (i32.const 31))))
    (call $get_str (i32.const 688) (i32.const 31))
    (call $set_slot (i32.const 27) (f64.convert_i32_s (call $cksum (i32.const 688))))
    ;; end of string: len 0
    (call $set_slot (i32.const 28)
      (f64.convert_i32_s (call $next_param (i32.const 640) (i32.const 31))))
"#;
    let bytes = guest_15(imports, data, calc_extra);
    let host = UserModelHost::load(
        "tier_c.wasm",
        &bytes,
        InterfaceKind::GenUserModel,
        HostConfig::default(),
    )
    .expect("load");

    let mut gvars = distinct_gen_vars();
    let mut dyn_rec = distinct_dyn_rec();
    let mut inst =
        UserModelInstance::new(&host, 1, sh!(&mut gvars, &mut dyn_rec)).expect("instance");
    let v = [Complex64::new(1.0, 0.0)];
    let mut i = [Complex64::new(0.0, 0.0)];
    inst.calc(&v, &mut i, sh!(&mut gvars, &mut dyn_rec))
        .expect("calc");

    let mut all = [0f64; 64];
    inst.get_all_vars(&mut all, sh!(&mut gvars, &mut dyn_rec))
        .expect("get_all_vars");
    let slot = |k: usize| all[k - 1];
    assert_eq!(
        slot(21),
        4.0,
        "NextParam returns the value length (\"3.25\")"
    );
    assert_eq!(
        slot(22),
        "kw".bytes().map(u32::from).sum::<u32>() as f64,
        "param name copied to the guest"
    );
    assert_eq!(slot(23), 3.25, "GetDblValue converts the current token");
    assert_eq!(slot(24), 1.0, "second value length (\"7\")");
    assert_eq!(slot(25), 7.0, "GetIntValue");
    assert_eq!(slot(26), 4.0, "third value length (\"fred\")");
    assert_eq!(
        slot(27),
        "fred".bytes().map(u32::from).sum::<u32>() as f64,
        "GetStrValue serves CB_Param from the last NextParam"
    );
    assert_eq!(slot(28), 0.0, "end of command string");
}

// ---------------------------------------------------------------------------
// Load-time validation: missing exports (the 569 path), signatures
// ---------------------------------------------------------------------------

#[test]
fn missing_export_names_the_exact_function() {
    for missing in [
        "new",
        "select",
        "calc",
        "integrate",
        "save",
        "get_var_name",
        "dss_alloc",
    ] {
        let bytes = stub_15_without(&[missing], &[]);
        let err = UserModelHost::load(
            "missing.wasm",
            &bytes,
            InterfaceKind::GenUserModel,
            HostConfig::default(),
        )
        .expect_err("load must fail");
        match err {
            UserModelError::MissingExport { name, model } => {
                assert_eq!(name, missing);
                assert_eq!(model, "missing.wasm");
            }
            other => panic!("expected MissingExport({missing}), got {other:?}"),
        }
    }
}

/// Several exports missing at once: the reported one follows the Pascal
/// binding order (`GenUserModel.pas:173-187` — `select` binds before `calc`).
#[test]
fn missing_export_reports_first_in_pascal_binding_order() {
    let bytes = stub_15_without(&["calc", "select"], &[]);
    let err = UserModelHost::load(
        "order.wasm",
        &bytes,
        InterfaceKind::GenUserModel,
        HostConfig::default(),
    )
    .expect_err("load must fail");
    assert!(
        matches!(err, UserModelError::MissingExport { name: "select", .. }),
        "got {err:?}"
    );
}

#[test]
fn missing_memory_export_is_reported() {
    let bytes = wat::parse_str(
        r#"(module (func (export "dss_alloc") (param i32) (result i32) (i32.const 0)))"#,
    )
    .expect("wat");
    let err = UserModelHost::load(
        "nomem.wasm",
        &bytes,
        InterfaceKind::GenUserModel,
        HostConfig::default(),
    )
    .expect_err("load must fail");
    assert!(
        matches!(err, UserModelError::MissingExport { name: "memory", .. }),
        "got {err:?}"
    );
}

/// The 13-function `TStoreDynaModel` interface accepts a guest without
/// `save`/`restore`; the 15-function interfaces reject the same module.
#[test]
fn thirteen_function_interface_needs_no_save_restore() {
    // The dynarec-form `new` the Storage shapes need, no save/restore.
    let bytes13 = {
        let wat = r#"(module (memory (export "memory") 1)
  (func (export "dss_alloc") (param i32) (result i32) (i32.const 64))
  (func (export "new") (param i32) (result i32) (i32.const 1))
  (func (export "select") (param i32) (result i32) (i32.const 1))
  (func (export "init") (param i32 i32))
  (func (export "calc") (param i32 i32))
  (func (export "integrate"))
  (func (export "edit") (param i32 i32))
  (func (export "update_model"))
  (func (export "delete") (param i32))
  (func (export "num_vars") (result i32) (i32.const 0))
  (func (export "get_all_vars") (param i32))
  (func (export "get_variable") (param i32) (result f64) (f64.const 0))
  (func (export "set_variable") (param i32 f64))
  (func (export "get_var_name") (param i32 i32 i32)))"#;
        wat::parse_str(wat).expect("wat")
    };

    // Loads as the 13-fn interface.
    UserModelHost::load(
        "dyna13.wasm",
        &bytes13,
        InterfaceKind::StoreDynaModel,
        HostConfig::default(),
    )
    .expect("13-fn interface must accept it");

    // The 15-fn Storage interface reports `save` (first missing in binding
    // order, `StoreUserModel.pas:214-228`).
    let err = UserModelHost::load(
        "dyna13.wasm",
        &bytes13,
        InterfaceKind::StoreUserModel,
        HostConfig::default(),
    )
    .expect_err("15-fn interface must reject it");
    assert!(
        matches!(err, UserModelError::MissingExport { name: "save", .. }),
        "got {err:?}"
    );
}

#[test]
fn wrong_export_signature_is_a_protocol_violation() {
    let bytes = stub_15_without(
        &[],
        &[("calc", r#"(func (export "calc") (param f64 f64))"#)],
    );
    let err = UserModelHost::load(
        "badsig.wasm",
        &bytes,
        InterfaceKind::GenUserModel,
        HostConfig::default(),
    )
    .expect_err("load must fail");
    match err {
        UserModelError::SignatureMismatch { name, .. } => assert_eq!(name, "calc"),
        other => panic!("expected SignatureMismatch, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Typed runtime failures: trap / fuel / OOB / memory cap / alloc / unsupported
// ---------------------------------------------------------------------------

#[test]
fn guest_trap_is_a_loud_typed_error_naming_model_and_function() {
    let bytes = guest_15("", "", "(unreachable)");
    let host = UserModelHost::load(
        "trap.wasm",
        &bytes,
        InterfaceKind::GenUserModel,
        HostConfig::default(),
    )
    .expect("load");
    let mut gvars = distinct_gen_vars();
    let mut dyn_rec = distinct_dyn_rec();
    let mut inst =
        UserModelInstance::new(&host, 1, sh!(&mut gvars, &mut dyn_rec)).expect("instance");
    let v = [Complex64::new(1.0, 0.0)];
    let mut i = [Complex64::new(0.0, 0.0)];
    let err = inst
        .calc(&v, &mut i, sh!(&mut gvars, &mut dyn_rec))
        .expect_err("calc must trap");
    match &err {
        UserModelError::Trap { model, func, .. } => {
            assert_eq!(model, "trap.wasm");
            assert_eq!(func, "calc");
        }
        other => panic!("expected Trap, got {other:?}"),
    }
    let msg = err.to_string();
    assert!(msg.contains("trap.wasm") && msg.contains("calc"), "{msg}");
}

#[test]
fn fuel_exhaustion_is_a_loud_typed_error() {
    let bytes = guest_15("", "", "(loop $spin (br $spin))");
    let host = UserModelHost::load(
        "fuel.wasm",
        &bytes,
        InterfaceKind::GenUserModel,
        HostConfig {
            fuel_per_call: 10_000,
            ..HostConfig::default()
        },
    )
    .expect("load");
    let mut gvars = distinct_gen_vars();
    let mut dyn_rec = distinct_dyn_rec();
    let mut inst =
        UserModelInstance::new(&host, 1, sh!(&mut gvars, &mut dyn_rec)).expect("instance");
    let v = [Complex64::new(1.0, 0.0)];
    let mut i = [Complex64::new(0.0, 0.0)];
    let err = inst
        .calc(&v, &mut i, sh!(&mut gvars, &mut dyn_rec))
        .expect_err("calc must exhaust fuel");
    assert!(
        matches!(
            &err,
            UserModelError::FuelExhausted { model, func }
                if model == "fuel.wasm" && func == "calc"
        ),
        "got {err:?}"
    );
}

#[test]
fn memory_cap_breach_is_a_loud_typed_error() {
    // 1 MiB cap; the guest tries to grow by 100 pages (6.4 MiB).
    let bytes = guest_15("", "", "(drop (memory.grow (i32.const 100)))");
    let host = UserModelHost::load(
        "memcap.wasm",
        &bytes,
        InterfaceKind::GenUserModel,
        HostConfig {
            memory_cap_bytes: 1024 * 1024,
            ..HostConfig::default()
        },
    )
    .expect("load");
    let mut gvars = distinct_gen_vars();
    let mut dyn_rec = distinct_dyn_rec();
    let mut inst =
        UserModelInstance::new(&host, 1, sh!(&mut gvars, &mut dyn_rec)).expect("instance");
    let v = [Complex64::new(1.0, 0.0)];
    let mut i = [Complex64::new(0.0, 0.0)];
    let err = inst
        .calc(&v, &mut i, sh!(&mut gvars, &mut dyn_rec))
        .expect_err("calc must breach the cap");
    assert!(
        matches!(
            &err,
            UserModelError::MemoryCapExceeded { model, func }
                if model == "memcap.wasm" && func == "calc"
        ),
        "got {err:?}"
    );
}

#[test]
fn dss_alloc_returning_zero_is_alloc_failed() {
    let bytes = stub_15_without(
        &[],
        &[(
            "dss_alloc",
            r#"(func (export "dss_alloc") (param i32) (result i32) (i32.const 0))"#,
        )],
    );
    let host = UserModelHost::load(
        "alloc0.wasm",
        &bytes,
        InterfaceKind::GenUserModel,
        HostConfig::default(),
    )
    .expect("load");
    let mut gvars = distinct_gen_vars();
    let mut dyn_rec = distinct_dyn_rec();
    let err = UserModelInstance::new(&host, 1, sh!(&mut gvars, &mut dyn_rec))
        .expect_err("instance creation must fail");
    assert!(
        matches!(&err, UserModelError::AllocFailed { model, .. } if model == "alloc0.wasm"),
        "got {err:?}"
    );
}

#[test]
fn dss_alloc_returning_out_of_range_pointer_is_out_of_bounds() {
    let bytes = stub_15_without(
        &[],
        &[(
            "dss_alloc",
            r#"(func (export "dss_alloc") (param i32) (result i32) (i32.const 0x0FF00000))"#,
        )],
    );
    let host = UserModelHost::load(
        "alloc_oob.wasm",
        &bytes,
        InterfaceKind::GenUserModel,
        HostConfig::default(),
    )
    .expect("load");
    let mut gvars = distinct_gen_vars();
    let mut dyn_rec = distinct_dyn_rec();
    let err = UserModelInstance::new(&host, 1, sh!(&mut gvars, &mut dyn_rec))
        .expect_err("instance creation must fail");
    assert!(
        matches!(&err, UserModelError::OutOfBounds { model, .. } if model == "alloc_oob.wasm"),
        "got {err:?}"
    );
}

/// An import handed an out-of-bounds destination records the typed OOB fault
/// (the typst `memory_error` pattern) and the call fails loudly.
#[test]
fn import_write_out_of_bounds_is_typed_oob() {
    let imports = r#"(import "dss_env" "get_dynamics_rec" (func $get_dyn (param i32)))"#;
    let calc_extra = "(call $get_dyn (i32.const 0x7FF00000))";
    let bytes = guest_15(imports, "", calc_extra);
    let host = UserModelHost::load(
        "cb_oob.wasm",
        &bytes,
        InterfaceKind::GenUserModel,
        HostConfig::default(),
    )
    .expect("load");
    let mut gvars = distinct_gen_vars();
    let mut dyn_rec = distinct_dyn_rec();
    let mut inst =
        UserModelInstance::new(&host, 1, sh!(&mut gvars, &mut dyn_rec)).expect("instance");
    let v = [Complex64::new(1.0, 0.0)];
    let mut i = [Complex64::new(0.0, 0.0)];
    let err = inst
        .calc(
            &v,
            &mut i,
            Shuttle {
                gen_vars: Some(&mut gvars),
                dyn_rec: &mut dyn_rec,
                ctx: Box::new(TierACtx::new()),
            },
        )
        .expect_err("calc must fail");
    match &err {
        UserModelError::OutOfBounds { model, func, .. } => {
            assert_eq!(model, "cb_oob.wasm");
            assert_eq!(func, "get_dynamics_rec");
        }
        other => panic!("expected OutOfBounds, got {other:?}"),
    }
}

/// The unsupported imports are loud attributed errors when called (ABI doc §4
/// rows 7/30/32) — never a silent no-op. `get_active_element_ptr` is permanent;
/// `do_dss_command`/`get_result_str` are loud here because this instance did
/// **not** opt into the WP-WM.6 deferred-command mechanism
/// (`enable_dss_commands` — the dss-rs element call sites likewise do not opt
/// in, so a real deck model hits this loud path rather than a silent drop).
#[test]
fn unsupported_imports_raise_loud_attributed_errors() {
    let cases: [(&str, &str, &str); 3] = [
        (
            r#"(import "dss_env" "do_dss_command" (func $f (param i32 i32)))"#,
            "(call $f (i32.const 0) (i32.const 0))",
            "do_dss_command",
        ),
        (
            r#"(import "dss_env" "get_result_str" (func $f (param i32 i32)))"#,
            "(call $f (i32.const 512) (i32.const 31))",
            "get_result_str",
        ),
        (
            r#"(import "dss_env" "get_active_element_ptr" (func $f (result i32)))"#,
            "(drop (call $f))",
            "get_active_element_ptr",
        ),
    ];
    for (imports, calc_extra, import_name) in cases {
        let bytes = guest_15(imports, "", calc_extra);
        let host = UserModelHost::load(
            "unsup.wasm",
            &bytes,
            InterfaceKind::GenUserModel,
            HostConfig::default(),
        )
        .expect("load");
        let mut gvars = distinct_gen_vars();
        let mut dyn_rec = distinct_dyn_rec();
        let mut inst =
            UserModelInstance::new(&host, 1, sh!(&mut gvars, &mut dyn_rec)).expect("instance");
        let v = [Complex64::new(1.0, 0.0)];
        let mut i = [Complex64::new(0.0, 0.0)];
        let err = inst
            .calc(&v, &mut i, sh!(&mut gvars, &mut dyn_rec))
            .expect_err("call must fail");
        assert!(
            matches!(
                &err,
                UserModelError::Unsupported { model, import }
                    if model == "unsup.wasm" && import == import_name
            ),
            "expected Unsupported({import_name}), got {err:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Callback tier C: the WP-WM.6 deferred DSS-command pair
// (do_dss_command / get_result_str)
// ---------------------------------------------------------------------------

/// The WP-WM.6 deferred-drain cycle end-to-end, with the test acting as the
/// re-entrant executive host (which the dss-rs element call sites cannot be —
/// ABI §4 rows 7/32):
///  1. the guest `calc` calls `do_dss_command("? Load.L1.kW")` then reads
///     `get_result_str` into a buffer (checksummed into slot 21);
///  2. the host `drain_dss_commands()` gets the command in order, "runs" it,
///     and `set_result_str(...)` captures the `GlobalResult`;
///  3. on the guest's *next* call the `get_result_str` serves that result —
///     the one observable ordering difference from Pascal's synchronous call
///     (the first call sees the pre-drain empty result).
#[test]
fn wm6_deferred_dss_command_cycle() {
    let imports = r#"
  (import "dss_env" "do_dss_command" (func $do_cmd (param i32 i32)))
  (import "dss_env" "get_result_str" (func $get_result (param i32 i32)))
"#;
    // "? Load.L1.kW" is 12 bytes at offset 400; the result buffer is at 512.
    let data = r#"(data (i32.const 400) "? Load.L1.kW")"#;
    let calc_extra = r#"
    (call $do_cmd (i32.const 400) (i32.const 12))
    (call $get_result (i32.const 512) (i32.const 63))
    (call $set_slot (i32.const 21) (f64.convert_i32_s (call $cksum (i32.const 512))))
"#;
    let bytes = guest_15(imports, data, calc_extra);
    let host = UserModelHost::load(
        "wm6cmd.wasm",
        &bytes,
        InterfaceKind::GenUserModel,
        HostConfig::default(),
    )
    .expect("load");

    let mut gvars = distinct_gen_vars();
    let mut dyn_rec = distinct_dyn_rec();
    let mut inst =
        UserModelInstance::new(&host, 1, sh!(&mut gvars, &mut dyn_rec)).expect("instance");
    inst.enable_dss_commands();

    let v = [Complex64::new(1.0, 0.0)];
    let mut i = [Complex64::new(0.0, 0.0)];

    // Call 1: the command is queued; get_result_str sees the empty pre-drain
    // result (the documented ordering difference).
    inst.calc(&v, &mut i, sh!(&mut gvars, &mut dyn_rec))
        .expect("calc 1");
    let cmds = inst.drain_dss_commands();
    assert_eq!(
        cmds,
        vec!["? Load.L1.kW".to_string()],
        "the guest command is queued verbatim, in order"
    );
    let mut all = [0f64; 64];
    inst.get_all_vars(&mut all, sh!(&mut gvars, &mut dyn_rec))
        .expect("get_all_vars 1");
    assert_eq!(
        all[20], 0.0,
        "get_result_str is empty before any command result is captured"
    );

    // The host runs the drained command through the executive and captures the
    // resulting GlobalResult.
    inst.set_result_str("3.25");

    // Call 2: get_result_str now serves the captured result.
    inst.calc(&v, &mut i, sh!(&mut gvars, &mut dyn_rec))
        .expect("calc 2");
    assert_eq!(
        inst.drain_dss_commands(),
        vec!["? Load.L1.kW".to_string()],
        "call 2 queues the command again"
    );
    inst.get_all_vars(&mut all, sh!(&mut gvars, &mut dyn_rec))
        .expect("get_all_vars 2");
    assert_eq!(
        all[20],
        "3.25".bytes().map(u32::from).sum::<u32>() as f64,
        "get_result_str serves the captured GlobalResult on the next call"
    );
    // Draining twice yields nothing (the queue was emptied).
    assert!(inst.drain_dss_commands().is_empty());
}

/// Multiple `do_dss_command` calls in one guest call queue in order, and the
/// `CapControlInstance` shares the same mechanism.
#[test]
fn wm6_deferred_commands_queue_in_order_on_cap_control() {
    let wat = r#"(module
  (import "dss_env" "do_dss_command" (func $do_cmd (param i32 i32)))
  (import "dss_env" "get_result_str" (func $get_result (param i32 i32)))
  (memory (export "memory") 1)
  (data (i32.const 100) "open capacitor.c")
  (data (i32.const 200) "? capacitor.c.states")
  (global $heap (mut i32) (i32.const 4096))
  (func (export "dss_alloc") (param $size i32) (result i32)
    (local $ptr i32)
    (local.set $ptr (global.get $heap))
    (global.set $heap (i32.add (global.get $heap) (local.get $size)))
    (local.get $ptr))
  (func (export "new") (result i32) (i32.const 4))
  (func (export "select") (param $id i32) (result i32) (local.get $id))
  (func (export "sample")
    (call $do_cmd (i32.const 100) (i32.const 16))
    (call $do_cmd (i32.const 200) (i32.const 20)))
  (func (export "do_pending") (param i32 i32)
    (call $get_result (i32.const 512) (i32.const 63)))
  (func (export "edit") (param i32 i32))
  (func (export "update_model"))
  (func (export "delete") (param i32)))"#;
    let bytes = wat::parse_str(wat).expect("wat");
    let host = UserModelHost::load(
        "wm6cap.wasm",
        &bytes,
        InterfaceKind::CapUserControl,
        HostConfig::default(),
    )
    .expect("load");

    let mut inst = CapControlInstance::new(&host, Box::new(NoCallbacks)).expect("instance");
    inst.enable_dss_commands();

    // Without opting in, the same guest would trap loudly; with it enabled the
    // two commands queue in order.
    inst.sample(Box::new(NoCallbacks)).expect("sample");
    assert_eq!(
        inst.drain_dss_commands(),
        vec![
            "open capacitor.c".to_string(),
            "? capacitor.c.states".to_string(),
        ],
        "both commands queued in order"
    );

    // do_pending reads the result the host captured after running them.
    inst.set_result_str("1 1 1");
    inst.do_pending(0, 0, Box::new(NoCallbacks))
        .expect("do_pending");
    // (No assertion on guest memory here — the CapControlInstance exposes no
    // var surface; the queue/serve wiring is covered by the Generator test.)
}

/// With the mechanism NOT enabled, a `CapControlInstance` guest that calls
/// `do_dss_command` fails loudly (parity with the Generator not-opted-in path).
#[test]
fn wm6_cap_control_do_dss_command_loud_without_opt_in() {
    let wat = r#"(module
  (import "dss_env" "do_dss_command" (func $do_cmd (param i32 i32)))
  (memory (export "memory") 1)
  (data (i32.const 100) "clear")
  (func (export "dss_alloc") (param i32) (result i32) (i32.const 64))
  (func (export "new") (result i32) (i32.const 1))
  (func (export "select") (param i32) (result i32) (i32.const 1))
  (func (export "sample") (call $do_cmd (i32.const 100) (i32.const 5)))
  (func (export "do_pending") (param i32 i32))
  (func (export "edit") (param i32 i32))
  (func (export "update_model"))
  (func (export "delete") (param i32)))"#;
    let bytes = wat::parse_str(wat).expect("wat");
    let host = UserModelHost::load(
        "wm6caploud.wasm",
        &bytes,
        InterfaceKind::CapUserControl,
        HostConfig::default(),
    )
    .expect("load");
    let mut inst = CapControlInstance::new(&host, Box::new(NoCallbacks)).expect("instance");
    let err = inst
        .sample(Box::new(NoCallbacks))
        .expect_err("do_dss_command must be loud without opt-in");
    assert!(
        matches!(&err, UserModelError::Unsupported { import, .. } if import == "do_dss_command"),
        "got {err:?}"
    );
    assert!(inst.drain_dss_commands().is_empty(), "nothing queued");
}

// ---------------------------------------------------------------------------
// Host API misuse
// ---------------------------------------------------------------------------

#[test]
fn save_on_13_function_interface_is_a_usage_error() {
    let wat = r#"(module (memory (export "memory") 2)
  (global $heap (mut i32) (i32.const 8192))
  (func (export "dss_alloc") (param $size i32) (result i32)
    (local $ptr i32)
    (local.set $ptr (global.get $heap))
    (global.set $heap (i32.add (global.get $heap) (local.get $size)))
    (local.get $ptr))
  (func (export "new") (param i32) (result i32) (i32.const 2))
  (func (export "select") (param $id i32) (result i32) (local.get $id))
  (func (export "init") (param i32 i32))
  (func (export "calc") (param i32 i32))
  (func (export "integrate"))
  (func (export "edit") (param i32 i32))
  (func (export "update_model"))
  (func (export "delete") (param i32))
  (func (export "num_vars") (result i32) (i32.const 0))
  (func (export "get_all_vars") (param i32))
  (func (export "get_variable") (param i32) (result f64) (f64.const 0))
  (func (export "set_variable") (param i32 f64))
  (func (export "get_var_name") (param i32 i32 i32)))"#;
    let bytes = wat::parse_str(wat).expect("wat");
    let host = UserModelHost::load(
        "dyna.wasm",
        &bytes,
        InterfaceKind::StoreDynaModel,
        HostConfig::default(),
    )
    .expect("load");
    let mut dyn_rec = distinct_dyn_rec();
    let mut inst = UserModelInstance::new(
        &host,
        1,
        Shuttle::without_gen_vars(&mut dyn_rec, Box::new(NoCallbacks)),
    )
    .expect("instance");
    assert_eq!(inst.id(), 2);
    let err = inst
        .save(Shuttle::without_gen_vars(
            &mut dyn_rec,
            Box::new(NoCallbacks),
        ))
        .expect_err("save must be rejected");
    assert!(matches!(err, UserModelError::Usage { .. }), "got {err:?}");
}

#[test]
fn vi_buffer_length_mismatch_is_a_usage_error() {
    let bytes = guest_15("", "", "");
    let host = UserModelHost::load(
        "vi.wasm",
        &bytes,
        InterfaceKind::GenUserModel,
        HostConfig::default(),
    )
    .expect("load");
    let mut gvars = distinct_gen_vars();
    let mut dyn_rec = distinct_dyn_rec();
    let mut inst =
        UserModelInstance::new(&host, 3, sh!(&mut gvars, &mut dyn_rec)).expect("instance");
    let v = [Complex64::new(1.0, 0.0); 2]; // yorder is 3
    let mut i = [Complex64::new(0.0, 0.0); 2];
    let err = inst
        .calc(&v, &mut i, sh!(&mut gvars, &mut dyn_rec))
        .expect_err("mismatched V/I must be rejected");
    assert!(matches!(err, UserModelError::Usage { .. }), "got {err:?}");
}

// ---------------------------------------------------------------------------
// CapControl 7-function interface
// ---------------------------------------------------------------------------

struct CapCtx;

impl Callbacks for CapCtx {
    fn control_queue_next_handle(&self) -> i32 {
        5
    }
}

#[test]
fn cap_control_seven_function_round_trip() {
    let wat = r#"(module
  (import "dss_env" "control_queue_push" (func $push (param i32 f64 i32 i32) (result i32)))
  (import "dss_env" "msg_callback" (func $msg (param i32 i32)))
  (memory (export "memory") 1)
  (global $heap (mut i32) (i32.const 4096))
  (func (export "dss_alloc") (param $size i32) (result i32)
    (local $ptr i32)
    (local.set $ptr (global.get $heap))
    (global.set $heap (i32.add (global.get $heap) (local.get $size)))
    (local.get $ptr))
  (func (export "new") (result i32) (i32.const 3))
  (func (export "select") (param $id i32) (result i32) (local.get $id))
  (func (export "sample")
    (drop (call $push (i32.const 0) (f64.const 15.0) (i32.const 2) (i32.const 1))))
  (func (export "do_pending") (param $code i32) (param $hdl i32)
    (drop (call $push (local.get $code) (f64.const 0) (local.get $hdl) (i32.const 0))))
  (func (export "edit") (param $ptr i32) (param $len i32)
    (call $msg (local.get $ptr) (local.get $len)))
  (func (export "update_model"))
  (func (export "delete") (param i32)))"#;
    let bytes = wat::parse_str(wat).expect("wat");
    let host = UserModelHost::load(
        "capctl.wasm",
        &bytes,
        InterfaceKind::CapUserControl,
        HostConfig::default(),
    )
    .expect("load");

    let mut inst = CapControlInstance::new(&host, Box::new(NoCallbacks)).expect("instance");
    assert_eq!(inst.id(), 3);
    assert!(inst.exists());
    assert_eq!(inst.model(), "capctl.wasm");

    assert_eq!(inst.select(Box::new(NoCallbacks)).expect("select"), 3);

    // edit echoes the UserData string back through the tier-B msg queue.
    inst.edit("deadband=2.0", Box::new(NoCallbacks))
        .expect("edit");
    assert_eq!(
        inst.drain_effects(),
        vec![Effect::Msg("deadband=2.0".to_string())]
    );

    inst.update_model(Box::new(NoCallbacks))
        .expect("update_model");

    // sample schedules a control action (CapControl.pas:1026-1041 pattern).
    inst.sample(Box::new(CapCtx)).expect("sample");
    assert_eq!(
        inst.drain_effects(),
        vec![Effect::ControlQueuePush {
            hour: 0,
            sec: 15.0,
            code: 2,
            proxy_hdl: 1,
            handle: 5,
        }]
    );

    // do_pending receives (code, proxy_hdl) by value (ABI doc §1).
    inst.do_pending(9, 4, Box::new(CapCtx)).expect("do_pending");
    assert_eq!(
        inst.drain_effects(),
        vec![Effect::ControlQueuePush {
            hour: 9,
            sec: 0.0,
            code: 4,
            proxy_hdl: 0,
            handle: 5,
        }]
    );

    inst.delete(Box::new(NoCallbacks)).expect("delete");
    assert!(!inst.exists());
}

#[test]
fn cap_control_kind_mismatch_is_a_usage_error() {
    // A 15-fn Generator guest loaded as CapControl fails validation on the
    // 7-fn export set first (`sample` binds after `select`,
    // CapUserControl.pas:176-182)…
    let bytes = guest_15("", "", "");
    let err = UserModelHost::load(
        "kindmix.wasm",
        &bytes,
        InterfaceKind::CapUserControl,
        HostConfig::default(),
    )
    .expect_err("15-fn Generator guest lacks the Cap `new()` shape");
    assert!(
        matches!(err, UserModelError::SignatureMismatch { ref name, .. } if name == "new"),
        "got {err:?}"
    );

    // …and a UserModelInstance over a CapControl host is host-API misuse.
    let wat = r#"(module (memory (export "memory") 1)
  (func (export "dss_alloc") (param i32) (result i32) (i32.const 64))
  (func (export "new") (result i32) (i32.const 1))
  (func (export "select") (param i32) (result i32) (i32.const 1))
  (func (export "sample"))
  (func (export "do_pending") (param i32 i32))
  (func (export "edit") (param i32 i32))
  (func (export "update_model"))
  (func (export "delete") (param i32)))"#;
    let bytes = wat::parse_str(wat).expect("wat");
    let host = UserModelHost::load(
        "cap.wasm",
        &bytes,
        InterfaceKind::CapUserControl,
        HostConfig::default(),
    )
    .expect("load");
    let mut dyn_rec = distinct_dyn_rec();
    let err = UserModelInstance::new(
        &host,
        1,
        Shuttle::without_gen_vars(&mut dyn_rec, Box::new(NoCallbacks)),
    )
    .expect_err("UserModelInstance over a CapControl host must be rejected");
    assert!(matches!(err, UserModelError::Usage { .. }), "got {err:?}");
}

/// `new` returning 0 is a creation failure: the instance reports
/// `exists() == false` (the engine then treats the model as absent —
/// Pascal `Get_Exists`).
#[test]
fn new_returning_zero_means_model_absent() {
    let wat = r#"(module (memory (export "memory") 2)
  (global $heap (mut i32) (i32.const 8192))
  (func (export "dss_alloc") (param $size i32) (result i32)
    (local $ptr i32)
    (local.set $ptr (global.get $heap))
    (global.set $heap (i32.add (global.get $heap) (local.get $size)))
    (local.get $ptr))
  (func (export "new") (param i32 i32) (result i32) (i32.const 0))
  (func (export "select") (param i32) (result i32) (i32.const 0))
  (func (export "init") (param i32 i32))
  (func (export "calc") (param i32 i32))
  (func (export "integrate"))
  (func (export "save"))
  (func (export "restore"))
  (func (export "edit") (param i32 i32))
  (func (export "update_model"))
  (func (export "delete") (param i32))
  (func (export "num_vars") (result i32) (i32.const 0))
  (func (export "get_all_vars") (param i32))
  (func (export "get_variable") (param i32) (result f64) (f64.const 0))
  (func (export "set_variable") (param i32 f64))
  (func (export "get_var_name") (param i32 i32 i32)))"#;
    let bytes = wat::parse_str(wat).expect("wat");
    let host = UserModelHost::load(
        "absent.wasm",
        &bytes,
        InterfaceKind::GenUserModel,
        HostConfig::default(),
    )
    .expect("load");
    let mut gvars = distinct_gen_vars();
    let mut dyn_rec = distinct_dyn_rec();
    let mut inst =
        UserModelInstance::new(&host, 1, sh!(&mut gvars, &mut dyn_rec)).expect("instance");
    assert_eq!(inst.id(), 0);
    assert!(!inst.exists());
    // Pascal `TGenUserModel.Edit` ignores edits while FID = 0.
    inst.edit("kw=1", sh!(&mut gvars, &mut dyn_rec))
        .expect("edit is a no-op");
}
