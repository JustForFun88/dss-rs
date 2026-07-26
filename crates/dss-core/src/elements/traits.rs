//! The circuit-element behavior trait — Pascal `TDSSCktElement`'s virtual
//! methods (`CalcYPrim`, `InjCurrents`, `GetCurrents`, `RecalcElementData`)
//! plus the context structs that replace "reach through `ActiveCircuit`"
//! global access (PORTING_PLAN.md §2.1).

use num_complex::Complex64;

use crate::elements::ckt::CktElementData;
use crate::elements::general::spectrum::SpectrumObj;
use crate::elements::meter::monitor::Monitor;
use crate::elements::pc::storage::Storage;
use crate::elements::pd::capacitor::Capacitor;
use crate::elements::pd::transformer::{ControlledTransformer, Transformer};
use crate::elements::pos_seq::{PosSeqCtx, PosSeqPlan};
use crate::obj::arena::{ArenaClass, ClassArena};
use crate::solution::SolveMode;
use crate::support::dynamics::IterationFlag;

/// The typed handle to an element inside the executive's class registry —
/// one enum variant per registered class, carrying a typed `Idx<T>` into that
/// class's arena. Re-exported here because [`ElemStore`] and every Pascal
/// pointer list (`CktElements`, `Sources`, `Lines`, `Loads`, ...) speak it:
/// the lists became `Vec<ElemId>`.
///
/// Before DE_PASCALIZE R3 this was an untyped `{ cls, idx }` tag struct;
/// [`ElemId::class_ord`]/[`ElemId::index`] expose the same two numbers
/// for the registry-side producers that still discover a class by position.
pub use crate::obj::arena::ElemId;

/// Element storage the solver walks — implemented by the executive's class
/// registry. Replaces Pascal's `TDSSPointerList` of `TDSSCktElement`.
///
/// `: Send` is the P7 thread-readiness rider (DE_PASCALIZE Part V / R1).
pub trait ElemStore: Send {
    fn ckt_elem(&self, r: ElemId) -> &dyn CktElement;
    fn ckt_elem_mut(&mut self, r: ElemId) -> &mut dyn CktElement;

    /// Read view of any registered object (control dispatch peeks at a
    /// control's references before splitting the mutable borrows).
    fn obj(&self, r: ElemId) -> &dyn crate::obj::base::DssObject;

    /// The [`ElemKind`] of the class the ref points at — the meter/sampling
    /// type-guards (`is_line`/`is_pd_element`/PD/PC checks) match on this
    /// instead of an `Any` downcast probe. Panics if `r` names a non-circuit
    /// ("general") class, which those guards never pass.
    ///
    /// [`ElemKind`]: crate::circuit::ElemKind
    fn kind(&self, r: ElemId) -> crate::circuit::ElemKind;

    /// Pascal `TDSSCircuit.SetElementActive`: resolve a full element name
    /// (`Class.Name`, or a bare `Name` searched across all circuit-element
    /// classes) to its [`ElemId`], or `None` if not found. Used by the
    /// EnergyMeter manual `ZoneList` zone build.
    fn find_ckt_element(&self, full_name: &str) -> Option<ElemId>;

    /// Pascal `<SomeClass>.Find(name)` reaching a *non-circuit* ("general",
    /// `DSS_OBJECT`) class registered via `DssClass::dss_object` — e.g.
    /// `XYcurve`. Unlike [`ElemStore::find_ckt_element`] this is **not**
    /// restricted to circuit-element classes. Used by
    /// `StorageController.Get_DynamicTarget`'s live, uncached
    /// `DSS.XYCurveClass.Find(DSS.SeasonSignal)` (the season signal is a bare
    /// `Set`-option string, not an object-ref property, so nothing can resolve
    /// and cache the `ElemId` up front at edit time).
    fn find_general(&self, class_name: &str, obj_name: &str) -> Option<ElemId>;

    /// Single mutable object view — the property/edit surface that does not
    /// need a concrete class (the concrete narrowing is [`TypedStore`]).
    fn obj_mut(&mut self, r: ElemId) -> &mut dyn crate::obj::base::DssObject;

    /// Two distinct objects borrowed mutably at once — the Rust stand-in for
    /// Pascal's live cross-object pointers during `Sample`/`DoPendingAction`
    /// (PHASE5_PLAN §2.1: the control plus its controlled element). Panics if
    /// `a == b`.
    fn pair_mut(
        &mut self,
        a: ElemId,
        b: ElemId,
    ) -> (
        &mut dyn crate::obj::base::DssObject,
        &mut dyn crate::obj::base::DssObject,
    );

    /// Three pairwise-distinct objects borrowed mutably at once (CapControl:
    /// control + capacitor + monitored element). Panics on any aliasing.
    fn triple_mut(
        &mut self,
        a: ElemId,
        b: ElemId,
        c: ElemId,
    ) -> (
        &mut dyn crate::obj::base::DssObject,
        &mut dyn crate::obj::base::DssObject,
        &mut dyn crate::obj::base::DssObject,
    );

    /// The typed arena of class ordinal `cls` — the storage the concrete
    /// accessors of [`TypedStore`] read through (`ClassArena::get::<T>`), with
    /// no `Any` round-trip. Dyn-safe on purpose: the generic narrowing lives in
    /// the [`TypedStore`] extension trait on top of it.
    fn arena(&self, cls: usize) -> &ClassArena;

    /// Mutable [`ElemStore::arena`].
    fn arena_mut(&mut self, cls: usize) -> &mut ClassArena;

    /// Two **distinct** class arenas borrowed mutably at once — the class-level
    /// half of the disjoint-borrow split ([`TypedStore`] does the object-level
    /// half). Panics if `a == b` or either index is out of range.
    fn arena_pair_mut(&mut self, a: usize, b: usize) -> (&mut ClassArena, &mut ClassArena);

    /// Three **pairwise-distinct** class arenas borrowed mutably at once.
    fn arena_triple_mut(
        &mut self,
        a: usize,
        b: usize,
        c: usize,
    ) -> (&mut ClassArena, &mut ClassArena, &mut ClassArena);
}

/// Concrete (`&T` / `&mut T`) access on top of [`ElemStore`] — the DE_PASCALIZE
/// R3 replacement for the removed `Any` downcast of `store.obj(r)` to `&T`.
///
/// Blanket-implemented for every `ElemStore` (including `dyn ElemStore`), so the
/// generic methods are available wherever the store is. Each one resolves the
/// class statically through [`ArenaClass`]: a handle naming another class is a
/// `None` from a compile-time match arm, exactly what the downcast returned.
pub trait TypedStore: ElemStore {
    /// The concrete `&T` behind `r`, or `None` if `r` names another class
    /// (the removed `Any` downcast of `store.obj(r)`).
    fn typed<T: ArenaClass>(&self, r: ElemId) -> Option<&T> {
        let i = T::idx_of(r)?;
        self.arena(T::CLASS_ORD).get::<T>(i.get())
    }

    /// The concrete `&mut T` behind `r` (the removed `Any` downcast of
    /// `obj_mut(r)`).
    fn typed_mut<T: ArenaClass>(&mut self, r: ElemId) -> Option<&mut T> {
        let i = T::idx_of(r)?;
        self.arena_mut(T::CLASS_ORD).get_mut::<T>(i.get())
    }

    /// A control plus its controlled element, borrowed disjointly: the control
    /// as its concrete class `C`, the target as `&mut dyn CktElement` (`None`
    /// when the target is a general/`DSS_OBJECT` class — the case the control
    /// dispatch reports as "… is not a circuit element").
    ///
    /// Panics on aliasing refs or a `C`-class mismatch, mirroring the
    /// `pair_mut` assert + the `expect("kind matched above")` the paired
    /// downcast carried.
    fn typed_ckt_pair_mut<C: ArenaClass>(
        &mut self,
        c: ElemId,
        t: ElemId,
    ) -> (&mut C, Option<&mut dyn CktElement>) {
        let (ci, ti) = pair_indices::<C>(c, t);
        if c.class_ord() == t.class_ord() {
            // Control and controlled element share a class arena.
            let (a, b) = split2::<C>(self.arena_mut(C::CLASS_ORD), ci, ti);
            (a, b.ckt_mut())
        } else {
            let (ca, ct) = self.arena_pair_mut(C::CLASS_ORD, t.class_ord());
            (expect_typed::<C>(ca, ci), ct.try_ckt_elem_mut(ti))
        }
    }

    /// [`TypedStore::typed_ckt_pair_mut`] plus the monitored element (also a
    /// `&mut dyn CktElement`) — the CapControl / Fuse / Relay / Recloser
    /// "control + controlled + monitored" triple. Panics on any aliasing or a
    /// `C`-class mismatch.
    #[allow(clippy::type_complexity)]
    fn typed_ckt_triple_mut<C: ArenaClass>(
        &mut self,
        c: ElemId,
        t: ElemId,
        m: ElemId,
    ) -> (
        &mut C,
        Option<&mut dyn CktElement>,
        Option<&mut dyn CktElement>,
    ) {
        let (ci, ti, mi) = triple_indices::<C>(c, t, m);
        let (tc, mc) = (t.class_ord(), m.class_ord());
        if C::CLASS_ORD == tc && tc == mc {
            let [a, b, d] = split3::<C>(self.arena_mut(C::CLASS_ORD), ci, ti, mi);
            (a, b.ckt_mut(), d.ckt_mut())
        } else if C::CLASS_ORD == tc {
            let (ct, cm) = self.arena_pair_mut(C::CLASS_ORD, mc);
            let (a, b) = split2::<C>(ct, ci, ti);
            (a, b.ckt_mut(), cm.try_ckt_elem_mut(mi))
        } else if C::CLASS_ORD == mc {
            let (cm, ct) = self.arena_pair_mut(C::CLASS_ORD, tc);
            let (a, d) = split2::<C>(cm, ci, mi);
            (a, ct.try_ckt_elem_mut(ti), d.ckt_mut())
        } else if tc == mc {
            let (ca, ctm) = self.arena_pair_mut(C::CLASS_ORD, tc);
            let (b, d) = ctm.pair_ckt_mut(ti, mi);
            (expect_typed::<C>(ca, ci), b, d)
        } else {
            let (ca, ct, cm) = self.arena_triple_mut(C::CLASS_ORD, tc, mc);
            (
                expect_typed::<C>(ca, ci),
                ct.try_ckt_elem_mut(ti),
                cm.try_ckt_elem_mut(mi),
            )
        }
    }

    /// RegControl's control/controlled pair: the controlled element resolves
    /// against **either** the Transformer or the AutoTrans class (the Pascal
    /// proxy), so it comes back as [`ControlledTransformer`]; `None` for any
    /// other class. Panics on aliasing or a `C`-class mismatch.
    fn typed_transformer_pair_mut<C: ArenaClass>(
        &mut self,
        c: ElemId,
        t: ElemId,
    ) -> (&mut C, Option<&mut dyn ControlledTransformer>) {
        let (ci, ti) = pair_indices::<C>(c, t);
        if c.class_ord() == t.class_ord() {
            // The control's own class is neither Transformer nor AutoTrans, so
            // a target in it is "not a Transformer or AutoTrans" — the same
            // outcome the per-class downcast chain produced.
            let (a, _) = split2::<C>(self.arena_mut(C::CLASS_ORD), ci, ti);
            return (a, None);
        }
        let (ca, ct) = self.arena_pair_mut(C::CLASS_ORD, t.class_ord());
        (
            expect_typed::<C>(ca, ci),
            ct.try_controlled_transformer_mut(ti),
        )
    }

    /// A control plus its controlled element **as a concrete class** `B`
    /// (CapControl → Capacitor), borrowed disjointly. `None` when the target
    /// names another class — the "Controlled element is not a …" path the
    /// paired downcast produced.
    ///
    /// `A` and `B` must be different classes (a control and the element it
    /// controls); asserted, since one arena cannot hand out two different
    /// concrete types.
    fn typed_pair_mut<A: ArenaClass, B: ArenaClass>(
        &mut self,
        a: ElemId,
        b: ElemId,
    ) -> (&mut A, Option<&mut B>) {
        assert_ne!(
            A::CLASS_ORD,
            B::CLASS_ORD,
            "typed_pair_mut: the two classes must differ"
        );
        let (ai, bi) = pair_indices::<A>(a, b);
        if b.class_ord() != B::CLASS_ORD {
            // `b` is not a `B`; the caller aborts on the `None`. Only the `A`
            // borrow is handed out (taking `b`'s too would need its arena and
            // would alias when `b` sits in `A`'s own arena).
            return (expect_typed::<A>(self.arena_mut(A::CLASS_ORD), ai), None);
        }
        let (ca, cb) = self.arena_pair_mut(A::CLASS_ORD, B::CLASS_ORD);
        (expect_typed::<A>(ca, ai), cb.get_mut::<B>(bi))
    }

    /// [`TypedStore::typed_pair_mut`] plus the monitored element as
    /// `&mut dyn CktElement` — CapControl's control + capacitor + monitored
    /// triple. Panics on any aliasing; `A` and `B` must be different classes.
    #[allow(clippy::type_complexity)]
    fn typed_triple_mut<A: ArenaClass, B: ArenaClass>(
        &mut self,
        a: ElemId,
        b: ElemId,
        m: ElemId,
    ) -> (&mut A, Option<&mut B>, Option<&mut dyn CktElement>) {
        assert_ne!(
            A::CLASS_ORD,
            B::CLASS_ORD,
            "typed_triple_mut: the two classes must differ"
        );
        let (ai, bi, mi) = triple_indices::<A>(a, b, m);
        let mc = m.class_ord();
        if b.class_ord() != B::CLASS_ORD {
            // `b` is not a `B` — hand back `A` + the monitored element only.
            return if mc == A::CLASS_ORD {
                let (x, mm) = split2::<A>(self.arena_mut(A::CLASS_ORD), ai, mi);
                (x, None, mm.ckt_mut())
            } else {
                let (ca, cm) = self.arena_pair_mut(A::CLASS_ORD, mc);
                (expect_typed::<A>(ca, ai), None, cm.try_ckt_elem_mut(mi))
            };
        }
        if mc == A::CLASS_ORD {
            let (ca, cb) = self.arena_pair_mut(A::CLASS_ORD, B::CLASS_ORD);
            let (x, mm) = split2::<A>(ca, ai, mi);
            (x, cb.get_mut::<B>(bi), mm.ckt_mut())
        } else if mc == B::CLASS_ORD {
            let (ca, cb) = self.arena_pair_mut(A::CLASS_ORD, B::CLASS_ORD);
            let (y, mm) = split2::<B>(cb, bi, mi);
            (expect_typed::<A>(ca, ai), Some(y), mm.ckt_mut())
        } else {
            let (ca, cb, cm) = self.arena_triple_mut(A::CLASS_ORD, B::CLASS_ORD, mc);
            (
                expect_typed::<A>(ca, ai),
                cb.get_mut::<B>(bi),
                cm.try_ckt_elem_mut(mi),
            )
        }
    }

    /// A [`Monitor`] plus the element it meters, borrowed disjointly, with the
    /// metered side already narrowed to the three concrete classes
    /// `TMonitorObj.TakeSample` reaches past [`CktElement`] for — mode 9
    /// `Capacitor.States`, mode 11 `Storage` present kW/kvar/kWh/state, modes
    /// 8/10 `Transformer.GetAllWindingCurrents`/`GetWindingVoltages`. Every
    /// other circuit class (including another Monitor, the same-arena case)
    /// arrives as [`MeteredElem::Other`], which is exactly what the per-class
    /// downcast chain returned `None` for.
    ///
    /// Panics on aliasing refs, a non-Monitor `c`, or a metered element that is
    /// not a circuit element (`Monitor.element=` resolves through
    /// `find_ckt_element`, so a general/`DSS_OBJECT` class cannot get here).
    fn typed_metered_pair_mut(&mut self, c: ElemId, t: ElemId) -> (&mut Monitor, MeteredElem<'_>) {
        let (ci, ti) = pair_indices::<Monitor>(c, t);
        if c.class_ord() == t.class_ord() {
            // A Monitor metering another Monitor: neither concrete arm applies.
            let (a, b) = split2::<Monitor>(self.arena_mut(Monitor::CLASS_ORD), ci, ti);
            return (a, MeteredElem::Other(b));
        }
        let (ca, ct) = self.arena_pair_mut(Monitor::CLASS_ORD, t.class_ord());
        let metered = if ct.get::<Capacitor>(ti).is_some() {
            MeteredElem::Capacitor(ct.get_mut::<Capacitor>(ti).expect("checked above"))
        } else if ct.get::<Storage>(ti).is_some() {
            MeteredElem::Storage(ct.get_mut::<Storage>(ti).expect("checked above"))
        } else if ct.get::<Transformer>(ti).is_some() {
            MeteredElem::Transformer(ct.get_mut::<Transformer>(ti).expect("checked above"))
        } else {
            MeteredElem::Other(
                ct.try_ckt_elem_mut(ti)
                    .expect("metered element is a circuit element"),
            )
        };
        (expect_typed::<Monitor>(ca, ci), metered)
    }
}

/// The element a [`Monitor`] meters, borrowed mutably and narrowed to the
/// concrete classes `TMonitorObj.TakeSample` needs beyond the [`CktElement`]
/// surface (see [`TypedStore::typed_metered_pair_mut`]). The typed R3
/// replacement for the `&mut dyn DssObject` + `Any`-downcast chain `take_sample`
/// used to take.
pub enum MeteredElem<'a> {
    Capacitor(&'a mut Capacitor),
    Storage(&'a mut Storage),
    Transformer(&'a mut Transformer),
    Other(&'a mut dyn CktElement),
}

impl MeteredElem<'_> {
    /// The generic circuit-element view (every arm has one).
    pub fn ckt(&self) -> &dyn CktElement {
        match self {
            MeteredElem::Capacitor(c) => *c,
            MeteredElem::Storage(s) => *s,
            MeteredElem::Transformer(t) => *t,
            MeteredElem::Other(e) => *e,
        }
    }

    /// Mutable [`MeteredElem::ckt`].
    pub fn ckt_mut(&mut self) -> &mut dyn CktElement {
        match self {
            MeteredElem::Capacitor(c) => *c,
            MeteredElem::Storage(s) => *s,
            MeteredElem::Transformer(t) => *t,
            MeteredElem::Other(e) => *e,
        }
    }

    /// The metered element as a `Capacitor` (mode 9), or `None`.
    pub fn capacitor(&self) -> Option<&Capacitor> {
        match self {
            MeteredElem::Capacitor(c) => Some(c),
            _ => None,
        }
    }

    /// The metered element as a `Storage` (mode 11), or `None`.
    pub fn storage(&self) -> Option<&Storage> {
        match self {
            MeteredElem::Storage(s) => Some(s),
            _ => None,
        }
    }

    /// The metered element as a `Transformer` (mode 8), or `None`.
    pub fn transformer(&self) -> Option<&Transformer> {
        match self {
            MeteredElem::Transformer(t) => Some(t),
            _ => None,
        }
    }

    /// Mutable [`MeteredElem::transformer`] (mode 10).
    pub fn transformer_mut(&mut self) -> Option<&mut Transformer> {
        match self {
            MeteredElem::Transformer(t) => Some(t),
            _ => None,
        }
    }
}

impl<S: ElemStore + ?Sized> TypedStore for S {}

/// Shared aliasing + class check of the typed pair getters (the `pair_mut`
/// assert and the `expect("kind matched above")` the downcast carried, in one
/// place). Returns the two object indices.
fn pair_indices<C: ArenaClass>(c: ElemId, t: ElemId) -> (usize, usize) {
    assert_ne!(
        (c.class_ord(), c.index()),
        (t.class_ord(), t.index()),
        "pair_mut: aliasing refs"
    );
    (expect_idx::<C>(c), t.index())
}

/// [`pair_indices`] for the three-object form.
fn triple_indices<C: ArenaClass>(c: ElemId, t: ElemId, m: ElemId) -> (usize, usize, usize) {
    let key = |r: ElemId| (r.class_ord(), r.index());
    assert!(
        key(c) != key(t) && key(c) != key(m) && key(t) != key(m),
        "triple_mut: aliasing refs"
    );
    (expect_idx::<C>(c), t.index(), m.index())
}

fn expect_idx<C: ArenaClass>(c: ElemId) -> usize {
    C::idx_of(c)
        .unwrap_or_else(|| {
            panic!(
                "expected a {} handle, got {}",
                C::CLASS_NAME,
                c.class_name()
            )
        })
        .get()
}

fn expect_typed<C: ArenaClass>(arena: &mut ClassArena, idx: usize) -> &mut C {
    arena
        .get_mut::<C>(idx)
        .unwrap_or_else(|| panic!("{} object #{idx} not in its arena", C::CLASS_NAME))
}

/// Two distinct objects of one `C` arena, borrowed mutably at once.
fn split2<C: ArenaClass>(arena: &mut ClassArena, i: usize, j: usize) -> (&mut C, &mut C) {
    let v = arena
        .all_mut::<C>()
        .unwrap_or_else(|| panic!("expected the {} arena", C::CLASS_NAME));
    let [a, b] = v
        .get_disjoint_mut([i, j])
        .expect("pair_mut: object index out of range");
    (a, b)
}

/// Three distinct objects of one `C` arena, borrowed mutably at once.
fn split3<C: ArenaClass>(arena: &mut ClassArena, i: usize, j: usize, k: usize) -> [&mut C; 3] {
    let v = arena
        .all_mut::<C>()
        .unwrap_or_else(|| panic!("expected the {} arena", C::CLASS_NAME));
    v.get_disjoint_mut([i, j, k])
        .expect("triple_mut: object index out of range")
}

/// Scalar state the elements read from the circuit/solution during
/// `CalcYPrim`/`InjCurrents` — the Pascal `ActiveCircuit.Solution.X`
/// accesses, snapshotted into one struct of plain values.
#[derive(Debug, Clone)]
pub struct SysCtx {
    /// `Solution.Frequency`.
    pub frequency: f64,
    /// `Circuit.Fundamental`.
    pub fundamental: f64,
    pub is_harmonic_model: bool,
    pub is_dynamic_model: bool,
    /// `Solution.LoadModel`: POWERFLOW (1) or ADMITTANCE (2).
    pub load_model: i32,
    pub mode: SolveMode,
    /// `Circuit.ActiveLoadShapeClass` (`Set LoadShapeClass=`): the class the
    /// GENERALTIME / DYNAMICMODE nominal dispatch consults (`USENONE`=-1 /
    /// `USEDAILY`=0 / `USEYEARLY`=1 / `USEDUTY`=2).
    pub active_load_shape_class: i32,
    /// `Circuit.LoadMultiplier`.
    pub load_multiplier: f64,
    /// `Circuit.GenMultiplier`.
    pub gen_multiplier: f64,
    /// `Circuit.GeneratorDispatchReference` (set per solve by
    /// `SetGeneratorDispRef`).
    pub generator_dispatch_reference: f64,
    /// `Circuit.PriceSignal` ($/MWh).
    pub price_signal: f64,
    /// `Circuit.DefaultGrowthFactor`.
    pub default_growth_factor: f64,
    /// `Solution.Year`.
    pub year: i32,
    /// `Solution.DynaVars.dblHour`.
    pub dbl_hour: f64,
    pub solution_count: i32,
    pub loads_need_updating: bool,
    pub neglect_load_y: bool,
    pub long_line_correction: bool,
    pub positive_sequence: bool,
    /// `Solution.TimeOfDay()` (no epsilon) — wrapped hour-of-day (Storage
    /// `CheckStateTriggerLevel` charge-time trigger).
    pub time_of_day: f64,
    /// `Solution.DynaVars.h` — the dynamics step size in seconds (Storage
    /// charge-time tolerance window).
    pub dyna_h: f64,
    /// `Solution.DynaVars.t` — seconds from the top of the hour (the dynamics
    /// clock the VCCS waveform integrator samples `w·t` against).
    pub dyna_t: f64,
    /// `Solution.DynaVars.IterationFlag` — the predictor (`NewTimeStep`) /
    /// corrector (`SameTimeStep`) selector consumed by `IntegrateStates`.
    pub iteration_flag: IterationFlag,
    /// `Solution.LastSolutionWasDirect` — set at the end of `SolveDirect`
    /// (`Solution.pas` l.1282), cleared at the end of `DoPFLOWsolution`
    /// (l.1022). While set, `TPCElement.GetCurrents` reports terminal currents
    /// via the `CalcYPrimContribution` shortcut ("the model is entirely in the
    /// Y matrix") instead of the load-model compensation current.
    pub last_solution_was_direct: bool,
}

impl SysCtx {
    /// Pascal `TPCElement.GetCurrents` shortcut condition (`PCElement.pas`
    /// l.137): `LastSolutionWasDirect and not (IsDynamicModel or
    /// IsHarmonicModel)` — take terminal currents from `YPrim · Vterminal`
    /// only. Applies to the PC classes that inherit the base `GetCurrents`
    /// (Load, Generator, IndMach012) and, via `inherited` in
    /// `TInvBasedPCE.GetCurrents` (`InvBasedPCE.pas` l.218), to non-GFM
    /// PVSystem/Storage; the overrides that never call `inherited` (Vsource,
    /// Isource, GICLine, GICsource, VCCS, UPFC, VSConverter, and the GFM
    /// branch of InvBasedPCE) must NOT consult it.
    pub fn pc_direct_shortcut(&self) -> bool {
        self.last_solution_was_direct && !(self.is_dynamic_model || self.is_harmonic_model)
    }

    /// The fresh-circuit parse-time snapshot: the state a brand-new circuit is in
    /// before any solve (60 Hz fundamental, `Mode=Snapshot`, unit multipliers,
    /// `dblHour=0`, `ActiveLoadShapeClass=USENONE`). It is the correct **fallback**
    /// only where no live circuit exists yet (a `DSS_OBJECT` edit before `New
    /// circuit`, whose `end_edit` ignores the context anyway); every circuit-element
    /// path threads the LIVE [`sys_ctx`](crate::solution::solution::sys_ctx) instead.
    /// Unit-test fixtures also use it as their default context.
    pub fn parse_default() -> Self {
        SysCtx {
            frequency: 60.0,
            fundamental: 60.0,
            is_harmonic_model: false,
            is_dynamic_model: false,
            load_model: crate::solution::POWERFLOW,
            mode: SolveMode::Snapshot,
            active_load_shape_class: crate::solution::USENONE,
            load_multiplier: 1.0,
            gen_multiplier: 1.0,
            generator_dispatch_reference: 0.0,
            price_signal: 25.0,
            default_growth_factor: 1.0,
            year: 0,
            dbl_hour: 0.0,
            solution_count: 0,
            loads_need_updating: true,
            neglect_load_y: false,
            long_line_correction: false,
            positive_sequence: false,
            time_of_day: 0.0,
            dyna_h: 0.0,
            dyna_t: 0.0,
            iteration_flag: IterationFlag::NewTimeStep,
            last_solution_was_direct: false,
        }
    }
}

/// Mutable solve-state the [`CktElement::compute_inj_currents`] fault path needs
/// *beyond* the (read-only) node-voltage vector, which is passed as a separate
/// parameter: the solve-time error sink and the abort flag. The current-injection
/// accumulator (`Solution.Currents`) and `SystemYChanged` are **gone** from this
/// context — each element fills its own element-owned `cd.inj_current` buffer and
/// returns the y-changed flag, and the caller scatters `cd.inj_current` into
/// `Solution.Currents` and ORs the flag sequentially. This is the
/// `MULTITHREADING_PLAN.md` M3b seam: the per-element compute performs no shared
/// `Currents` write, so the elements can later compute in parallel into their own
/// buffers. (`errors`/`solution_abort` remain a shared `&mut` for now — the
/// per-element collection of those is M3b's own step, DE_PASCALIZE R2 rider note.)
pub struct InjComputeCtx<'a> {
    /// `DoSimpleMsg` sink for a solve-time model fault surfaced from inside
    /// `compute_inj_currents` — a WASM user-model **trap**/protocol fault, or a
    /// `Model=6` Generator with no user model (`#567`/`#5671`,
    /// `generator.pas:1834/1943`). The inject loop drains this into the solution
    /// `ErrorLog`, so the fault is a loud typed error, **never a silent fallback**
    /// (WASM_USERMODELS plan §2.9-5): the terminal-sink compute method has no
    /// `Result`, so this is the channel that carries the diagnostic out.
    pub errors: &'a mut crate::diag::ErrorLog,
    /// `Solution.SolutionAbort`. A hard model fault raised from
    /// `compute_inj_currents` (Pascal `DoErrorMsg` / `DSS.SolutionAbort := TRUE`) —
    /// e.g. the missing dynamics model (`#5671`, `generator.pas:1944`) or >3-phase
    /// dynamics (`:2023`). Set from an `abort`-flagged diagnostic when the loop
    /// drains [`Self::errors`], so the surrounding solve stops instead of iterating
    /// on a best-effort stale current.
    pub solution_abort: &'a mut bool,
}

/// Per-element reliability inputs returned by [`CktElement::reliability_data`]
/// for the EnergyMeter reliability sweep (Pascal `TPDElement` fields).
#[derive(Debug, Clone, Copy, Default)]
pub struct ReliabilityData {
    /// `BranchFltRate` = `CalcFltRate` result (faults/yr for this branch).
    pub branch_flt_rate: f64,
    /// `HrsToRepair`: average repair time (hours).
    pub hrs_to_repair: f64,
    /// `MilesThisLine`: branch length in miles (0 for non-line PD elements).
    pub miles_this_line: f64,
}

/// Pascal `TDSSCktElement` virtual surface (Phase 3 subset).
///
/// `: Send` is the P7 thread-readiness rider (DE_PASCALIZE Part V / R1).
pub trait CktElement: Send {
    fn cd(&self) -> &CktElementData;
    fn cd_mut(&mut self) -> &mut CktElementData;

    /// Pascal `TDSSCktElement.SetNodeRef` (virtual): copy one terminal's node
    /// refs into the flat array + terminal record. The base behavior is the
    /// `CktElementData` method; `TAutoTransObj` overrides it to alias the series
    /// winding's second node onto the common winding's first ("Magic happens
    /// here", `AutoTrans.pas:875`). The circuit build path calls this (not
    /// `cd_mut().set_node_ref`) so the override fires.
    fn set_node_ref(&mut self, iterm: usize, node_ref_array: &[usize]) {
        self.cd_mut().set_node_ref(iterm, node_ref_array);
    }

    /// `CalcYPrim` (abstract): rebuild the primitive Y matrices.
    fn calc_yprim(&mut self, sys: &SysCtx);

    /// `InjCurrents` minus the scatter — the `MULTITHREADING_PLAN.md` M3b seam
    /// (sources and PC elements). Compute this element's injection into its own
    /// element-owned `cd.inj_current` buffer and return whether it invalidated its
    /// own YPrim (Pascal `set_YprimInvalid` raising `Solution.SystemYChanged`,
    /// `CktElement.pas` l.245). The **caller** then scatters `cd.inj_current` into
    /// `Solution.Currents` through `NodeRef` (slot 0 absorbs ground) and ORs the
    /// returned flag into `Solution.SystemYChanged`, sequentially and in element
    /// order — so the sums are bit-identical to the old fused `InjCurrents`, while
    /// the per-element compute itself performs no shared `Currents` write. The base
    /// class raises the Pascal "Improper call" error; PD elements never get called.
    fn compute_inj_currents(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
        ctx: &mut InjComputeCtx,
    ) -> bool {
        let _ = (sys, node_v, ctx);
        unreachable!(
            "Improper call to InjCurrents for Element: \"{}\"",
            self.cd().obj.name()
        );
    }

    /// Pascal `TPCElement.InitHarmonics`: capture the per-element harmonic base
    /// values (the fundamental-frequency reference magnitude/angle the spectrum
    /// is applied to) from the present fundamental solution. Run once over every
    /// enabled PC element when entering harmonics mode (`InitializeForHarmonics`).
    /// Default no-op — most elements (and the sources, whose harmonic injection
    /// is recomputed each step) carry no harmonic state.
    fn init_harmonics(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        let _ = (sys, node_v);
    }

    /// Pascal `TPCElement.InitStateVars` (`PCElement.pas` l.174): seed this
    /// machine's dynamic state variables from the present (power-flow) operating
    /// point. Run once over every enabled PC element when entering dynamics mode
    /// (`calcInitialMachineStates`, the `OK_for_Dynamics` success path). Default
    /// no-op — the base `TPCElement` and the elements without dynamic state
    /// (loads, sources) carry nothing to initialise.
    fn init_state_vars(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        let _ = (sys, node_v);
    }

    /// Pascal `TPCElement.IntegrateStates` (`PCElement.pas` l.179): advance this
    /// machine's dynamic states by one predictor or corrector half-step (the
    /// `Solution.iteration_flag` predictor/corrector selector is surfaced to
    /// `SysCtx` once a machine consumes it, WP7.7 step 2). Run over every PC
    /// element twice per dynamics time step (`IntegratePCStates`). Default no-op
    /// — only machines with dynamic state respond.
    fn integrate_states(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        let _ = (sys, node_v);
    }

    /// Pascal `TPCElement.NumVariables` (`PCElement.pas`): the number of dynamic
    /// state variables this element exposes (Monitor mode 3 reads them). Default
    /// 0 — non-machine elements carry no state variables.
    fn num_variables(&self) -> usize {
        0
    }

    /// Pascal `TPCElement.VariableName(i)` (`PCElement.pas`): the name of state
    /// variable `i` (1-based). Default empty — only machines name their states.
    fn variable_name(&self, i: usize) -> String {
        let _ = i;
        String::new()
    }

    /// Pascal `TPCElement.GetAllVariables(States)` (`PCElement.pas`): fill `states`
    /// (length ≥ [`Self::num_variables`]) with the present value of every dynamic
    /// state variable. Default no-op — non-machine elements write nothing. Run by
    /// Monitor mode 3 each dynamics sample.
    fn get_all_variables(&mut self, sys: &SysCtx, node_v: &[Complex64], states: &mut [f64]) {
        let _ = (sys, node_v, states);
    }

    /// Pascal `TPCElement.Set_Variable(i, value)` (`PCElement.pas`): write dynamic
    /// state variable `i` (1-based). The write side of the variable interface
    /// (`num_variables`/`variable_name`/`get_all_variables`), mirroring the
    /// `TPCElement` virtual. Default no-op — only machines with settable state
    /// respond. `sys` is the LIVE circuit/solution snapshot (the `Set` command
    /// site holds it): a write routed to a WASM user model feeds it into the guest
    /// callbacks exactly as Pascal hands the model live `ActiveCircuit.Solution`
    /// pointers; the built-in setters ignore it.
    fn set_variable(&mut self, i: usize, value: f64, sys: &SysCtx) {
        let _ = (i, value, sys);
    }

    /// Pascal `SpectrumObj`: the harmonic spectrum this element injects from, if
    /// one is resolved. Read by the harmonic frequency sweep
    /// (`CollectAllFrequencies`). Default None.
    fn harmonic_spectrum(&self) -> Option<&SpectrumObj> {
        None
    }

    /// The `spectrum=` name this element resolves its harmonic spectrum from
    /// (default or explicit), or None if it has no spectrum. The executive
    /// resolves it at edit-completion (Pascal `Set_Spectrum` / the constructor
    /// default) and hands the clone back through [`Self::set_harmonic_spectrum`].
    fn harmonic_spectrum_name(&self) -> Option<&str> {
        None
    }

    /// Store the resolved harmonic spectrum snapshot (Pascal `SpectrumObj`).
    fn set_harmonic_spectrum(&mut self, spectrum: Option<SpectrumObj>) {
        let _ = spectrum;
    }

    /// Pascal `GetSourceFrequency` (Vsource/Isource): the source's own base
    /// frequency, used by `CollectAllFrequencies` for the source pass. Non-source
    /// elements return None (the sweep uses the system fundamental for them).
    fn source_frequency(&self) -> Option<f64> {
        None
    }

    /// `GetCurrents`: total currents into the element terminals. The default
    /// is the PD-element behavior `Iterminal = Yprim · Vterminal`; PC
    /// elements override (compensation form).
    fn get_currents(&mut self, sys: &SysCtx, node_v: &[Complex64], curr: &mut [Complex64]) {
        let _ = sys;
        let cd = self.cd_mut();
        if !cd.enabled || cd.node_ref.is_empty() {
            curr.fill(Complex64::ZERO);
            return;
        }
        cd.compute_vterminal(node_v);
        if let Some(yprim) = &cd.yprim {
            yprim.mv_mult(curr, &cd.vterminal);
        } else {
            curr.fill(Complex64::ZERO);
        }
    }

    /// `ComputeIterminal`: cache-aware terminal-current refresh.
    fn compute_iterminal(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        if !self.cd().iterminal_solved_for(sys.solution_count) {
            let mut curr = vec![Complex64::ZERO; self.cd().yorder];
            self.get_currents(sys, node_v, &mut curr);
            let cd = self.cd_mut();
            cd.iterminal.copy_from_slice(&curr);
            cd.mark_iterminal_solved(sys.solution_count);
        }
    }

    /// Force a fresh `Iterminal` from the present `NodeV`, bypassing the
    /// `SolutionCount` cache — the model of the CAPI `CktElement.Currents` read
    /// path (`CAPI_CktElement.pas` `elem.GetCurrents`), which always recomputes
    /// `Yprim·Vterminal (± inj)` rather than returning the solver's internal
    /// `ComputeIterminal` cache. The two agree after every fixed-point solve
    /// (the cache is invalid at read time, so `compute_iterminal` recomputes),
    /// but `DoNewtonSolution`'s final `SumAllCurrents` stamps `Iterminal` at the
    /// converged `SolutionCount` from the *pre-final* voltage guess `NodeV_{n-1}`
    /// (the update `NodeV -= dV` follows it), so a plain `compute_iterminal`
    /// would then return that one-step-stale current. Reporting reads use this
    /// to match the oracle's fresh `GetCurrents`.
    fn refresh_iterminal(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        let mut curr = vec![Complex64::ZERO; self.cd().yorder];
        self.get_currents(sys, node_v, &mut curr);
        let cd = self.cd_mut();
        cd.iterminal.copy_from_slice(&curr);
        cd.mark_iterminal_solved(sys.solution_count);
    }

    /// Pascal `TPDElement.IsShunt`: true for shunt-connected capacitors and
    /// reactors (`Circuit.Get_Losses` ignores shunt PD elements). The base
    /// class default is false.
    fn is_shunt(&self) -> bool {
        false
    }

    /// Pascal `(pElem is TInvBasedPCE) and TInvBasedPCE(pElem).GFM_Mode` — an
    /// inverter-based PC element (PVSystem/Storage) currently in grid-forming
    /// mode. The solution splits its injection pass on this flag
    /// (`GetPCInjCurr(GFMOnly)`): a GFM PCE injects with the *sources*, not with
    /// the ordinary PC elements. Default false.
    fn is_gfm(&self) -> bool {
        false
    }

    /// Pascal `TControlElem.FControlledElement` (via `Set_ControlledElement`):
    /// the circuit element this control acts on, or `None` for a non-control
    /// element (and for the fleet controls that act on a *list* of elements
    /// rather than a single one). The reverse of Pascal's
    /// `ControlledElement.ControlElementList` — the reports that need the
    /// forward `PDElement → controls` mapping (`ShowControlledElements`,
    /// `ShowTopology`) derive it by scanning `Circuit.controls` and matching this.
    /// `Circuit.controls` is in creation order, so the derived per-element list
    /// reproduces the Pascal `ControlElementList` insertion order, and a control
    /// reassigned to a different target follows its *current* target — the same
    /// final state as Pascal's remove-then-add `Set_ControlledElement`. **Known
    /// narrow limitation:** when a control's element ref is *re-edited* after a
    /// second control already registered on the same target, Pascal's remove-then-
    /// add re-appends the re-edited control to the *end* of that target's list,
    /// whereas the creation-order derive keeps the original order — so the two
    /// disagree only for ≥2 controls on one element with a post-creation
    /// element-ref edit (probe-only; no corpus deck hits it — the fully-faithful
    /// fix would materialise the whole `ControlElementList`, disproportionate here).
    /// Default `None`; every control overrides it to return
    /// `self.ccd.controlled_element`.
    fn controlled_element(&self) -> Option<ElemId> {
        None
    }

    /// Per-element reliability inputs for the EnergyMeter reliability sweep
    /// (Pascal `TPDElement.CalcFltRate` + the `HrsToRepair`/`MilesThisLine`
    /// fields). `CalcFltRate` is virtual: the base `TPDElement` formula is
    /// `FaultRate · pctperm · 0.01`, which `TLineObj` overrides by multiplying
    /// by `Len`. Non-PD elements return the zero default and never appear in a
    /// meter `SequenceList`.
    fn reliability_data(&self) -> ReliabilityData {
        ReliabilityData::default()
    }

    /// Pascal `TDSSCktElement.GetTermVoltages(iTerm, VBuffer)`: the node voltages
    /// at terminal `iterm` (1-based) into `vbuffer` (0-based, length ≥ nconds);
    /// zeros if the terminal number is out of range. Used by the controls to
    /// sense a monitored element's terminal voltages.
    fn get_term_voltages(&self, iterm: usize, node_v: &[Complex64], vbuffer: &mut [Complex64]) {
        let cd = self.cd();
        let ncond = cd.nconds;
        if iterm < 1 || iterm > cd.nterms || cd.node_ref.is_empty() {
            for v in vbuffer.iter_mut().take(ncond) {
                *v = Complex64::ZERO;
            }
            return;
        }
        for (vb, &n) in vbuffer.iter_mut().zip(cd.term_nodes(iterm - 1)) {
            *vb = node_v[n];
        }
    }

    /// Pascal `TDSSCktElement.Get_Power(idxTerm)`: total complex power (W, var)
    /// into terminal `idx_term` (1-based), summed over its conductors (zero refs
    /// skipped), ×3 under positive sequence.
    fn terminal_power(&mut self, sys: &SysCtx, node_v: &[Complex64], idx_term: usize) -> Complex64 {
        if !self.cd().enabled || self.cd().node_ref.is_empty() {
            return Complex64::ZERO;
        }
        self.compute_iterminal(sys, node_v);
        let cd = self.cd();
        let mut result = Complex64::ZERO;
        for (&n, &ci) in cd
            .term_nodes(idx_term - 1)
            .iter()
            .zip(cd.term_i(idx_term - 1))
        {
            if n > 0 {
                result += node_v[n] * ci.conj();
            }
        }
        if sys.positive_sequence {
            result *= 3.0;
        }
        result
    }

    /// `Get_Losses`: sum of `NodeV[ref] · conj(Iterminal)` over all
    /// conductors (zero refs skipped), ×3 under positive sequence.
    fn losses(&mut self, sys: &SysCtx, node_v: &[Complex64]) -> Complex64 {
        if !self.cd().enabled || self.cd().node_ref.is_empty() {
            return Complex64::ZERO;
        }
        self.compute_iterminal(sys, node_v);
        let cd = self.cd();
        let mut result = Complex64::ZERO;
        for (&n, &ci) in cd.node_ref.iter().zip(&cd.iterminal) {
            if n > 0 {
                result += node_v[n] * ci.conj();
            }
        }
        if sys.positive_sequence {
            result *= 3.0;
        }
        result
    }

    /// `NormAmps` rating (PD elements override; 0 = no rating, like Pascal's
    /// base where `Get_ExcesskVANorm` short-circuits to 0).
    fn norm_amps(&self) -> f64 {
        0.0
    }

    /// `EmergAmps` rating (PD elements override).
    fn emerg_amps(&self) -> f64 {
        0.0
    }

    /// `NumAmpRatings` — the number of seasonal current ratings (`Seasons`).
    /// `1` for a base PDElement / non-PD element (no seasonal ratings).
    fn num_amp_ratings(&self) -> i32 {
        1
    }

    /// `AmpRatings` — the per-season current ratings array (PD elements with
    /// `Seasons > 1` override). Empty by default.
    fn amp_ratings(&self) -> &[f64] {
        &[]
    }

    /// Pascal `TPDElement.GetRatings` (EPRI r4133 PDElement.pas l.351): the
    /// (norm, emerg) current ratings, overridden by the seasonal rating
    /// `AmpRatings[seasonal_idx]` when the global season index is in range AND the
    /// element carries more than one season. r4133's guard is
    /// `(RatingIdx <= NumAmpRatings) and (NumAmpRatings > 1)`; a single-season
    /// element (`NumAmpRatings == 1`) keeps its base `(NormAmps, EmergAmps)`.
    /// dss_capi 0.15.x `55400a29` DROPPED the `NumAmpRatings > 1` guard, so on
    /// capi015 a single-season element at idx 0 silently replaces its user-set
    /// ratings with the stale constructor default `AmpRatings[0]` — proven a bug
    /// vs r4133 + physics (0.15.x-adoption sweep, DIVERGENCES L4/E2), so the port
    /// follows r4133 and keeps the `> 1` guard. We keep the memory-safe
    /// `0 <= idx < NumAmpRatings` bound (r4133's own `<= NumAmpRatings` is an
    /// off-the-end dynamic-array read — a UB defect not reproduced). Both norm and
    /// emerg take the same seasonal value.
    fn get_ratings(&self, seasonal_idx: i32) -> (f64, f64) {
        let norm = self.norm_amps();
        let emerg = self.emerg_amps();
        if self.num_amp_ratings() > 1 && seasonal_idx >= 0 && seasonal_idx < self.num_amp_ratings()
        {
            let r = self
                .amp_ratings()
                .get(seasonal_idx as usize)
                .copied()
                .unwrap_or(norm);
            (r, r)
        } else {
            (norm, emerg)
        }
    }

    /// Pascal `TDSSCktElement.MaxTerminalOneIMag` (CktElement.pas l.552): the
    /// max phase-current magnitude on terminal 1. Forces `Iterminal`.
    fn max_terminal_one_imag(&mut self, sys: &SysCtx, node_v: &[Complex64]) -> f64 {
        if !self.cd().enabled || self.cd().node_ref.is_empty() {
            return 0.0;
        }
        self.compute_iterminal(sys, node_v);
        let cd = self.cd();
        let mut max_sq = 0.0_f64;
        for i in 0..cd.nphases {
            let c = cd.iterminal[i];
            max_sq = max_sq.max(c.re * c.re + c.im * c.im);
        }
        max_sq.sqrt()
    }

    /// Pascal `TPDElement.Get_ExcessKVANorm` (PDElement.pas l.230): excess kVA
    /// over the normal rating into `idx_term` (1-based), in kVA. Side effect:
    /// sets `overload_een` to the per-unit overload factor.
    fn excess_kva_norm(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
        idx_term: usize,
    ) -> Complex64 {
        let norm_amps = self.norm_amps();
        if norm_amps == 0.0 || !self.cd().enabled {
            self.cd_mut().overload_een = 0.0;
            return Complex64::ZERO;
        }
        let kva = self.terminal_power(sys, node_v, idx_term) * 0.001; // forces Iterminal
        let imax = self.max_terminal_one_imag(sys, node_v);
        let factor = imax / norm_amps - 1.0;
        if factor > 0.0 {
            self.cd_mut().overload_een = factor;
            kva * (1.0 - 1.0 / (factor + 1.0))
        } else {
            self.cd_mut().overload_een = 0.0;
            Complex64::ZERO
        }
    }

    /// Pascal `TPDElement.Get_ExcessKVAEmerg` (PDElement.pas l.257). Side
    /// effect: sets `overload_ue`.
    fn excess_kva_emerg(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
        idx_term: usize,
    ) -> Complex64 {
        let emerg_amps = self.emerg_amps();
        if emerg_amps == 0.0 || !self.cd().enabled {
            self.cd_mut().overload_ue = 0.0;
            return Complex64::ZERO;
        }
        let kva = self.terminal_power(sys, node_v, idx_term) * 0.001;
        let imax = self.max_terminal_one_imag(sys, node_v);
        let factor = imax / emerg_amps - 1.0;
        if factor > 0.0 {
            self.cd_mut().overload_ue = factor;
            kva * (1.0 - 1.0 / (factor + 1.0))
        } else {
            self.cd_mut().overload_ue = 0.0;
            Complex64::ZERO
        }
    }

    /// Pascal `TDSSCktElement.GetLosses` (CktElement.pas l.441): total, load and
    /// no-load losses (W, var). Base default returns `(total, total, 0)`;
    /// Transformer/Reactor override to split the no-load component.
    fn get_losses_split(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
    ) -> (Complex64, Complex64, Complex64) {
        let total = self.losses(sys, node_v);
        (total, total, Complex64::ZERO)
    }

    /// Pascal `TDSSCktElement.GetSeqLosses` (base l.1092): sequence-mode losses.
    /// Base returns zeros; Line overrides for 3-phase branches.
    fn get_seq_losses(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
    ) -> (Complex64, Complex64, Complex64) {
        let _ = (sys, node_v);
        (Complex64::ZERO, Complex64::ZERO, Complex64::ZERO)
    }

    /// Pascal `TDSSCktElement.MakePosSequence` (virtual): convert this element
    /// to its positive-sequence equivalent (`TExecHelper.DoMakePosSeq` calls it
    /// on every circuit element, in creation order, after setting
    /// `PositiveSequence := TRUE`). The element mutates its own direct fields
    /// here and returns the ordered property-system mutations the exec applier
    /// must replay through the typed setter helpers (see [`PosSeqPlan`]).
    ///
    /// The default is the base behavior: no property sets, run the base bus
    /// rename ([`CktElementData::make_pos_sequence_base`]) — every element
    /// without a `MakePosSequence` override in Pascal inherits exactly this.
    ///
    /// [`PosSeqPlan`]: crate::elements::pos_seq::PosSeqPlan
    /// [`CktElementData::make_pos_sequence_base`]: crate::elements::ckt::CktElementData::make_pos_sequence_base
    fn make_pos_sequence(&mut self, ctx: &PosSeqCtx) -> PosSeqPlan {
        let _ = ctx;
        PosSeqPlan::default()
    }

    /// Pascal `TLineObj` length in kilometres (`Len · <units→km>`). `None` for
    /// every non-Line element — the EnergyMeter zone walk adds it to
    /// `DistFromMeter` only for lines (R0 Category B typed read, replacing an
    /// per-class guard on `store.obj`).
    fn line_length_km(&self) -> Option<f64> {
        None
    }

    /// Pascal `TLoadObj.NumCustomers`. `None` for every non-Load element — the
    /// EnergyMeter zone walk counts customers and appends to the load list only
    /// for loads (R0 Category B typed read).
    fn load_num_customers(&self) -> Option<i32> {
        None
    }

    /// Pascal transformer / autotransformer `PresentTap[iWinding]` (Monitor
    /// mode 2, the tap monitor). `None` for every non-transformer element (the
    /// mode records `0.0`). `terminal` is the 1-based winding index.
    fn present_tap(&self, terminal: usize) -> Option<f64> {
        let _ = terminal;
        None
    }

    /// Pascal `TControlElem.MonitoredElement` / `TMeterElement.MeteredElement`:
    /// the element this control/meter senses, resolved to its [`ElemId`]. The
    /// exec applier reads it to build the [`PosSeqCtx::monitored`] snapshot
    /// before calling [`Self::make_pos_sequence`]. Default `None` — a plain
    /// circuit element monitors nothing; controls/meters override it (in the
    /// later WTs of this round).
    ///
    /// [`PosSeqCtx::monitored`]: crate::elements::pos_seq::PosSeqCtx::monitored
    fn monitored_element_ref(&self) -> Option<ElemId> {
        None
    }
}
