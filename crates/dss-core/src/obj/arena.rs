//! Typed element arenas — the `PORTING_PLAN.md §2.1` storage design the port
//! originally shortcut with a heterogeneous `Vec<Box<dyn DssObject>>` per class.
//!
//! This is DE_PASCALIZE **R1**: the typed-arena storage. Each registered class
//! gets its own `Vec<T>` of the concrete element type, held inside a
//! [`ClassArena`] enum variant. The ownership flip (R1 commit 2) puts one
//! `ClassArena` on each `DssClass` (`DssClass::arena`), replacing the pre-R1
//! `Vec<Box<dyn DssObject>>` — this keeps every `classes: &[DssClass]` view and
//! the `ClassStore`/`ForeignClasses` borrow split unchanged (objects reached via
//! `class.arena` instead of `class.objects`).
//!
//! An [`ElemId`] enum (one `Idx<T>` variant per class) is the R2 typed handle
//! that will replace the `{cls, idx}` [`ElemRef`] tag across the spine; R1 only
//! introduces it (plus the ordering proof) and keeps the spine on `ElemRef`.
//! [`Elements`] is the corresponding hoisted aggregate (`Vec<ClassArena>` +
//! whole-registry accessors) that R2 can adopt once the spine is retyped; today
//! it backs the [`tests`] ordering proof and the `Send` assertions.
//!
//! **The single most important invariant:** the arena/variant order is exactly
//! the `exec/construct.rs` class-registration order. Bare-name
//! `find_ckt_element` and `ForeignClasses` lookups iterate classes in
//! registration order and return the first match, so the arena layout and every
//! iteration over it MUST preserve that order. The macro below emits everything
//! from ONE class list in that order; [`tests`] proves it against the live
//! registry.
//!
//! The typed `Vec<T>` inside each variant is the parallelism substrate
//! `MULTITHREADING_PLAN.md` M3 needs (`ClassArena::Line(v) => v.par_iter_mut()`);
//! it is exposed directly (`ClassArena`'s variants / [`ClassArena::objs_mut`],
//! [`Elements::arenas_mut`] / [`Elements::for_each_ckt_elem_mut`]), never
//! funnelled through a single `&mut dyn ElemStore` (Part V thread-readiness).
//!
//! `Idx<T>` is stable for the whole life of a circuit: OpenDSS never deletes an
//! individual element mid-script (`Clear` drops the entire circuit and resets
//! every arena together), so an index never dangles or shifts.

use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::ops::{Index, IndexMut};

use crate::elements::traits::{CktElement, ElemRef};
use crate::obj::base::DssObject;

/// A typed, stable index into the [`Elements`] arena for concrete type `T`
/// (Pascal had a bare integer; this is `PORTING_PLAN §2.1`'s `Idx<T>`).
///
/// Stable because OpenDSS never removes a single element mid-script — see the
/// module docs. `PhantomData<fn() -> T>` makes `Idx<T>` `Copy`/`Send`/`Sync`
/// regardless of `T` and keeps the manual trait impls free of a `T` bound.
pub struct Idx<T>(u32, PhantomData<fn() -> T>);

impl<T> Idx<T> {
    /// Wrap a 0-based arena position.
    pub fn new(i: usize) -> Self {
        Idx(i as u32, PhantomData)
    }

    /// The 0-based arena position.
    pub fn get(self) -> usize {
        self.0 as usize
    }
}

// Hand-written so the impls do not carry a `T: Clone`/`Copy`/… bound (the
// concrete element types are not `Copy`, but the index is).
impl<T> Clone for Idx<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Idx<T> {}
impl<T> PartialEq for Idx<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl<T> Eq for Idx<T> {}
impl<T> Hash for Idx<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}
impl<T> fmt::Debug for Idx<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Idx({})", self.0)
    }
}

/// The one class list, in `exec/construct.rs` registration order (the invariant
/// the whole module rests on). Columns: canonical class name (as the registry
/// reports it), `ElemId`/`ClassArena` variant, concrete element type.
///
/// Any new class must be appended here in the same position it is registered in
/// `construct.rs` — [`tests::arena_order_matches_registry`] fails otherwise.
macro_rules! with_all_classes {
    ($m:ident) => {
        $m! {
            // ── DSS_OBJECT (general / data) classes ──
            "TCC_Curve"         TccCurve          crate::elements::general::tcc_curve::TccCurveObj;
            "Spectrum"          Spectrum          crate::elements::general::spectrum::SpectrumObj;
            "LineCode"          LineCode          crate::elements::general::line_code::LineCodeObj;
            "GrowthShape"       GrowthShape       crate::elements::general::growth_shape::GrowthShapeObj;
            "XfmrCode"          XfmrCode          crate::elements::general::xfmr_code::XfmrCodeObj;
            "XYcurve"           XyCurve           crate::elements::general::xy_curve::XyCurveObj;
            "LoadShape"         LoadShape         crate::elements::general::load_shape::LoadShapeObj;
            "TShape"            TShape            crate::elements::general::temp_shape::TShapeObj;
            "PriceShape"        PriceShape        crate::elements::general::price_shape::PriceShapeObj;
            "WireData"          WireData          crate::elements::general::conductor_data::WireDataObj;
            "CNData"            CnData            crate::elements::general::conductor_data::CnDataObj;
            "TSData"            TsData            crate::elements::general::conductor_data::TsDataObj;
            "LineSpacing"       LineSpacing       crate::elements::general::line_spacing::LineSpacingObj;
            "LineGeometry"      LineGeometry      crate::elements::general::line_geometry::LineGeometryObj;
            "DynamicExp"        DynamicExp        crate::elements::general::dynamic_exp::DynamicExpObj;
            // ── circuit-element classes ──
            "Vsource"           Vsource           crate::elements::pc::vsource::VSource;
            "Isource"           Isource           crate::elements::pc::isource::Isource;
            "Line"              Line              crate::elements::pd::line::Line;
            "Load"              Load              crate::elements::pc::load::Load;
            "Transformer"       Transformer       crate::elements::pd::transformer::Transformer;
            "Capacitor"         Capacitor         crate::elements::pd::capacitor::Capacitor;
            "Reactor"           Reactor           crate::elements::pd::reactor::Reactor;
            "Fault"             Fault             crate::elements::pd::fault::Fault;
            "RegControl"        RegControl        crate::elements::control::reg_control::RegControl;
            "CapControl"        CapControl        crate::elements::control::cap_control::CapControl;
            "Generator"         Generator         crate::elements::pc::generator::Generator;
            "WindGen"           WindGen           crate::elements::pc::windgen::WindGen;
            "GenDispatcher"     GenDispatcher     crate::elements::control::gen_dispatcher::GenDispatcher;
            "StorageController" StorageController  crate::elements::control::storage_controller::StorageController;
            "Relay"             Relay             crate::elements::control::relay::Relay;
            "Recloser"          Recloser          crate::elements::control::recloser::Recloser;
            "Fuse"              Fuse              crate::elements::pd::fuse::Fuse;
            "SwtControl"        SwtControl        crate::elements::control::swt_control::SwtControl;
            "Storage"           Storage           crate::elements::pc::storage::Storage;
            "PVSystem"          PVSystem          crate::elements::pc::pvsystem::PVSystem;
            "UPFC"              Upfc              crate::elements::pc::upfc::Upfc;
            "UPFCControl"       UpfcControl       crate::elements::control::upfc_control::UpfcControl;
            "ESPVLControl"      EspvlControl      crate::elements::control::espvl_control::EspvlControl;
            "IndMach012"        IndMach012        crate::elements::pc::ind_mach012::IndMach012;
            "GICsource"         GicSource         crate::elements::pc::gic_source::GicSource;
            "AutoTrans"         AutoTrans         crate::elements::pd::auto_trans::AutoTrans;
            "VSConverter"       VsConverter       crate::elements::pc::vs_converter::VsConverter;
            "VCCS"              Vccs              crate::elements::pc::vccs::Vccs;
            "InvControl"        InvControl        crate::elements::control::inv_control::InvControl;
            "ExpControl"        ExpControl        crate::elements::control::exp_control::ExpControl;
            "GICLine"           GicLine           crate::elements::pc::gic_line::GicLine;
            "GICTransformer"    GicTransformer    crate::elements::pd::gic_transformer::GicTransformer;
            "Monitor"           Monitor           crate::elements::meter::monitor::Monitor;
            "EnergyMeter"       EnergyMeter       crate::elements::meter::energymeter::EnergyMeter;
            "Sensor"            Sensor            crate::elements::meter::sensor::Sensor;
        }
    };
}

/// The one consumer macro: expands the class list into `enum ElemId`,
/// `enum ClassArena`, and their per-variant match-arm impls (`obj`/`obj_mut`/
/// `ckt_elem`/`ckt_elem_mut`/`len`/`class_name`/`push_new`/`pair_mut_same`/
/// `triple_mut_same`/`for_each_ckt_elem_mut`) plus `ElemId::CLASS_NAMES`.
macro_rules! define_arena {
    ( $( $cname:literal $variant:ident $ty:ty ; )* ) => {
        /// The R2 typed element handle: one variant per registered class, each
        /// carrying a typed [`Idx<T>`]. Variant order == registration order (see
        /// module docs); [`ElemId::CLASS_NAMES`] pins it against the registry.
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
        pub enum ElemId {
            $( $variant(Idx<$ty>), )*
        }

        impl ElemId {
            /// The registry class names, in registration order — the proof
            /// surface for [`tests::arena_order_matches_registry`].
            pub const CLASS_NAMES: &'static [&'static str] = &[ $( $cname, )* ];

            /// This handle's class name (Pascal `TDSSClass.Name`).
            pub fn class_name(self) -> &'static str {
                match self {
                    $( ElemId::$variant(_) => $cname, )*
                }
            }

            /// The 0-based class index (position in registration order == the
            /// `ElemRef::cls` tag).
            pub fn class_ord(self) -> usize {
                match self {
                    $( ElemId::$variant(_) => Self::CLASS_NAMES.iter()
                        .position(|&n| n == $cname)
                        .expect("variant name is in CLASS_NAMES"), )*
                }
            }

            /// The 0-based object index within the class arena.
            pub fn index(self) -> usize {
                match self {
                    $( ElemId::$variant(i) => i.get(), )*
                }
            }

            /// The `{cls, idx}` [`ElemRef`] this handle denotes (the R1
            /// transition bridge — the spine still speaks `ElemRef`).
            pub fn to_ref(self) -> ElemRef {
                ElemRef { cls: self.class_ord(), idx: self.index() }
            }
        }

        /// One class's live objects — a typed `Vec<T>` of the concrete element
        /// type, held per registration-order slot in [`Elements::arenas`].
        pub enum ClassArena {
            $( $variant(Vec<$ty>), )*
        }

        impl ClassArena {
            /// An empty arena for the class named `cname` (case-insensitive) —
            /// how [`DssClass`] builds its arena at registration, keyed off its
            /// own `props.class_name()`.
            ///
            /// [`DssClass`]: crate::exec::registry::DssClass
            pub fn empty_for(cname: &str) -> Option<Self> {
                $( if cname.eq_ignore_ascii_case($cname) {
                    return Some(ClassArena::$variant(Vec::new()));
                } )*
                None
            }

            /// Drop every object (Pascal `Clear`; the whole arena resets at
            /// once — the `Idx<T>` stability invariant).
            pub fn clear(&mut self) {
                match self {
                    $( ClassArena::$variant(v) => v.clear(), )*
                }
            }

            /// This arena's class name (Pascal `TDSSClass.Name`).
            pub fn class_name(&self) -> &'static str {
                match self {
                    $( ClassArena::$variant(_) => $cname, )*
                }
            }

            /// Number of live objects in this class.
            pub fn len(&self) -> usize {
                match self {
                    $( ClassArena::$variant(v) => v.len(), )*
                }
            }

            pub fn is_empty(&self) -> bool {
                self.len() == 0
            }

            /// Read view of object `idx` as `&dyn DssObject`. The `+ 'static`
            /// object bound holds (every concrete element type owns its data,
            /// no borrows) and lets [`ClassArena`]'s `Index` use it directly.
            pub fn obj(&self, idx: usize) -> &(dyn DssObject + 'static) {
                match self {
                    $( ClassArena::$variant(v) => &v[idx], )*
                }
            }

            /// Mutable view of object `idx` as `&mut dyn DssObject`.
            pub fn obj_mut(&mut self, idx: usize) -> &mut (dyn DssObject + 'static) {
                match self {
                    $( ClassArena::$variant(v) => &mut v[idx], )*
                }
            }

            /// Bounds-checked read view of object `idx` (the typed twin of the
            /// old `objects.get(idx)`).
            pub fn get(&self, idx: usize) -> Option<&(dyn DssObject + 'static)> {
                (idx < self.len()).then(|| self.obj(idx))
            }

            /// Iterate every object as `&dyn DssObject`, in creation order (the
            /// typed twin of `objects.iter()`). Boxed because each variant's
            /// concrete `Vec<T>` iterator has its own type; only report/admin
            /// paths use it, never the solve hot loop.
            pub fn objs(&self) -> Box<dyn Iterator<Item = &(dyn DssObject + 'static)> + '_> {
                match self {
                    $( ClassArena::$variant(v) => {
                        Box::new(v.iter().map(|o| o as &(dyn DssObject + 'static)))
                    } )*
                }
            }

            /// Iterate every object as `&mut dyn DssObject`, in creation order
            /// (the typed twin of `objects.iter_mut()`).
            pub fn objs_mut(
                &mut self,
            ) -> Box<dyn Iterator<Item = &mut (dyn DssObject + 'static)> + '_> {
                match self {
                    $( ClassArena::$variant(v) => {
                        Box::new(v.iter_mut().map(|o| o as &mut (dyn DssObject + 'static)))
                    } )*
                }
            }

            /// Circuit-element read view of object `idx`; panics for a general
            /// (`DSS_OBJECT`) class, mirroring the old `ClassStore` `.expect`.
            pub fn ckt_elem(&self, idx: usize) -> &dyn CktElement {
                self.obj(idx)
                    .as_ckt_element()
                    .expect("ElemRef must point at a circuit element")
            }

            /// Circuit-element mutable view of object `idx`.
            pub fn ckt_elem_mut(&mut self, idx: usize) -> &mut dyn CktElement {
                self.obj_mut(idx)
                    .as_ckt_element_mut()
                    .expect("ElemRef must point at a circuit element")
            }

            /// Construct a fresh all-default object of this class named `name`,
            /// push it, and return its 0-based index (Pascal `NewObject`; the
            /// typed equivalent of `(DssClass.new_object)(name)`).
            pub fn push_new(&mut self, name: &str) -> usize {
                match self {
                    $( ClassArena::$variant(v) => {
                        v.push(<$ty>::new(name));
                        v.len() - 1
                    } )*
                }
            }

            /// Pascal `MakeLike`: copy `source`'s state into `target`, both in
            /// this (same) class. `clone_box` snapshots the source first so the
            /// target can be borrowed mutably afterwards (works even if
            /// `source == target`).
            pub fn make_like_within(&mut self, target: usize, source: usize) {
                match self {
                    $( ClassArena::$variant(v) => {
                        let src = v[source].clone_box();
                        v[target].make_like(src.as_ref());
                    } )*
                }
            }

            /// Two distinct objects of *this* arena, borrowed mutably at once.
            pub(crate) fn pair_mut_same(
                &mut self,
                i: usize,
                j: usize,
            ) -> (&mut dyn DssObject, &mut dyn DssObject) {
                match self {
                    $( ClassArena::$variant(v) => {
                        let [a, b] = v
                            .get_disjoint_mut([i, j])
                            .expect("pair_mut: object index out of range");
                        (a as &mut dyn DssObject, b as &mut dyn DssObject)
                    } )*
                }
            }

            /// Three distinct objects of *this* arena, borrowed mutably at once.
            pub(crate) fn triple_mut_same(
                &mut self,
                i: usize,
                j: usize,
                k: usize,
            ) -> (&mut dyn DssObject, &mut dyn DssObject, &mut dyn DssObject) {
                match self {
                    $( ClassArena::$variant(v) => {
                        let [a, b, c] = v
                            .get_disjoint_mut([i, j, k])
                            .expect("triple_mut: object index out of range");
                        (
                            a as &mut dyn DssObject,
                            b as &mut dyn DssObject,
                            c as &mut dyn DssObject,
                        )
                    } )*
                }
            }

            /// Two distinct objects — one from this arena, one from `other`
            /// (a different class), borrowed mutably at once.
            fn pair_mut_cross<'a>(
                &'a mut self,
                i: usize,
                other: &'a mut ClassArena,
                j: usize,
            ) -> (&'a mut dyn DssObject, &'a mut dyn DssObject) {
                (self.obj_mut(i), other.obj_mut(j))
            }

            /// Run `f` over every *circuit* element of this class as
            /// `&mut dyn CktElement`, tagged with its [`ElemRef`]. General
            /// (`DSS_OBJECT`) objects are skipped. The typed `Vec<T>` is the
            /// iteration substrate `MULTITHREADING_PLAN.md` M3 later swaps to
            /// `par_iter_mut`.
            fn for_each_ckt_elem_mut(
                &mut self,
                cls: usize,
                f: &mut dyn FnMut(ElemRef, &mut dyn CktElement),
            ) {
                match self {
                    $( ClassArena::$variant(v) => {
                        for (idx, o) in v.iter_mut().enumerate() {
                            if let Some(ce) = o.as_ckt_element_mut() {
                                f(ElemRef { cls, idx }, ce);
                            }
                        }
                    } )*
                }
            }
        }

        /// Build the empty arenas in registration order — the storage twin of
        /// the `exec/construct.rs` class list.
        fn empty_arenas() -> Vec<ClassArena> {
            vec![ $( ClassArena::$variant(Vec::new()), )* ]
        }
    };
}

with_all_classes!(define_arena);

// Index a `ClassArena` by object position, yielding `dyn DssObject` — the
// drop-in shape for the pre-R1 `objects[idx]` place expression (so the ownership
// flip is a near-rename of `<class>.objects[i]` → `<arena>[i]`, `IndexMut`
// auto-selecting when a `&mut` is needed).
impl Index<usize> for ClassArena {
    type Output = dyn DssObject;
    fn index(&self, idx: usize) -> &(dyn DssObject + 'static) {
        self.obj(idx)
    }
}
impl IndexMut<usize> for ClassArena {
    fn index_mut(&mut self, idx: usize) -> &mut (dyn DssObject + 'static) {
        self.obj_mut(idx)
    }
}

/// The whole registry's live objects: one [`ClassArena`] per registered class,
/// `arenas[class_index]` (Pascal's per-`TDSSClass` `ElementList`, collected).
///
/// `arenas` is indexable and `split_at_mut`-able by the class index — the same
/// shape as `Vec<DssClass>` — which is what lets the property-edit borrow split
/// (`command.rs` `edit_active_inner`) keep working after the ownership flip.
pub struct Elements {
    pub(crate) arenas: Vec<ClassArena>,
}

impl Default for Elements {
    fn default() -> Self {
        Self::new()
    }
}

impl Elements {
    /// Empty arenas in registration order.
    pub fn new() -> Self {
        Elements {
            arenas: empty_arenas(),
        }
    }

    /// Number of live objects in class `cls`.
    pub fn len(&self, cls: usize) -> usize {
        self.arenas[cls].len()
    }

    pub fn is_class_empty(&self, cls: usize) -> bool {
        self.arenas[cls].is_empty()
    }

    /// Read view of object `(cls, idx)`.
    pub fn obj(&self, cls: usize, idx: usize) -> &dyn DssObject {
        self.arenas[cls].obj(idx)
    }

    /// Mutable view of object `(cls, idx)`.
    pub fn obj_mut(&mut self, cls: usize, idx: usize) -> &mut dyn DssObject {
        self.arenas[cls].obj_mut(idx)
    }

    /// Circuit-element read view of object `(cls, idx)`.
    pub fn ckt_elem(&self, cls: usize, idx: usize) -> &dyn CktElement {
        self.arenas[cls].ckt_elem(idx)
    }

    /// Circuit-element mutable view of object `(cls, idx)`.
    pub fn ckt_elem_mut(&mut self, cls: usize, idx: usize) -> &mut dyn CktElement {
        self.arenas[cls].ckt_elem_mut(idx)
    }

    /// Construct + push a fresh object of class `cls` named `name`; returns its
    /// 0-based index.
    pub fn push_new(&mut self, cls: usize, name: &str) -> usize {
        self.arenas[cls].push_new(name)
    }

    /// Two pairwise-distinct objects borrowed mutably at once — the typed-arena
    /// form of the old `ClassStore::pair_mut`. Panics on aliasing refs.
    pub fn pair_mut(&mut self, a: ElemRef, b: ElemRef) -> (&mut dyn DssObject, &mut dyn DssObject) {
        pair_mut_arenas(&mut self.arenas, a, b)
    }

    /// Three pairwise-distinct objects borrowed mutably at once. Panics on any
    /// aliasing.
    pub fn triple_mut(
        &mut self,
        a: ElemRef,
        b: ElemRef,
        c: ElemRef,
    ) -> (&mut dyn DssObject, &mut dyn DssObject, &mut dyn DssObject) {
        triple_mut_arenas(&mut self.arenas, a, b, c)
    }

    /// The per-class arenas as a slice — direct typed access for
    /// `par_iter_mut`-style consumers (`MULTITHREADING_PLAN.md` M3).
    pub fn arenas(&self) -> &[ClassArena] {
        &self.arenas
    }

    pub fn arenas_mut(&mut self) -> &mut [ClassArena] {
        &mut self.arenas
    }

    /// Run `f` over every circuit element in the whole registry, in
    /// registration order, as `&mut dyn CktElement` tagged with its
    /// [`ElemRef`]. Do not funnel new bulk work through a single
    /// `&mut dyn ElemStore` entry point — use this or [`Self::arenas_mut`]
    /// (Part V thread-readiness rider).
    pub fn for_each_ckt_elem_mut(&mut self, mut f: impl FnMut(ElemRef, &mut dyn CktElement)) {
        for (cls, arena) in self.arenas.iter_mut().enumerate() {
            arena.for_each_ckt_elem_mut(cls, &mut f);
        }
    }
}

/// Two pairwise-distinct objects borrowed mutably at once, over a
/// `[ClassArena]` slice (shared by [`Elements`] and the `ClassStore` view).
pub(crate) fn pair_mut_arenas(
    arenas: &mut [ClassArena],
    a: ElemRef,
    b: ElemRef,
) -> (&mut dyn DssObject, &mut dyn DssObject) {
    assert_ne!((a.cls, a.idx), (b.cls, b.idx), "pair_mut: aliasing refs");
    if a.cls == b.cls {
        arenas[a.cls].pair_mut_same(a.idx, b.idx)
    } else {
        let [ca, cb] = arenas
            .get_disjoint_mut([a.cls, b.cls])
            .expect("pair_mut: class index out of range");
        ca.pair_mut_cross(a.idx, cb, b.idx)
    }
}

/// Three pairwise-distinct objects borrowed mutably at once, over a
/// `[ClassArena]` slice. Splits per distinct class first, then per object
/// inside a shared class (the same case analysis as the old `ClassStore`).
pub(crate) fn triple_mut_arenas(
    arenas: &mut [ClassArena],
    a: ElemRef,
    b: ElemRef,
    c: ElemRef,
) -> (&mut dyn DssObject, &mut dyn DssObject, &mut dyn DssObject) {
    let key = |r: ElemRef| (r.cls, r.idx);
    assert!(
        key(a) != key(b) && key(a) != key(c) && key(b) != key(c),
        "triple_mut: aliasing refs"
    );
    if a.cls == b.cls && b.cls == c.cls {
        arenas[a.cls].triple_mut_same(a.idx, b.idx, c.idx)
    } else if a.cls == b.cls {
        let [cab, cc] = arenas
            .get_disjoint_mut([a.cls, c.cls])
            .expect("triple_mut: class index out of range");
        let (oa, ob) = cab.pair_mut_same(a.idx, b.idx);
        (oa, ob, cc.obj_mut(c.idx))
    } else if a.cls == c.cls {
        let [cac, cb] = arenas
            .get_disjoint_mut([a.cls, b.cls])
            .expect("triple_mut: class index out of range");
        let (oa, oc) = cac.pair_mut_same(a.idx, c.idx);
        (oa, cb.obj_mut(b.idx), oc)
    } else if b.cls == c.cls {
        let [ca, cbc] = arenas
            .get_disjoint_mut([a.cls, b.cls])
            .expect("triple_mut: class index out of range");
        let (ob, oc) = cbc.pair_mut_same(b.idx, c.idx);
        (ca.obj_mut(a.idx), ob, oc)
    } else {
        let [ca, cb, cc] = arenas
            .get_disjoint_mut([a.cls, b.cls, c.cls])
            .expect("triple_mut: class index out of range");
        (ca.obj_mut(a.idx), cb.obj_mut(b.idx), cc.obj_mut(c.idx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exec::Dss;

    /// The load-bearing R1 invariant: the arena/`ElemId` variant order is
    /// exactly the `exec/construct.rs` registration order. A drift here silently
    /// breaks bare-name `find_ckt_element` / `ForeignClasses` tie-breaking.
    #[test]
    fn arena_order_matches_registry() {
        let dss = Dss::new();
        let names = dss.registered_class_names();
        let elems = Elements::new();

        assert_eq!(
            ElemId::CLASS_NAMES.len(),
            names.len(),
            "ElemId covers a different class count than the registry"
        );
        assert_eq!(
            elems.arenas.len(),
            names.len(),
            "Elements has a different arena count than the registry"
        );

        for (i, reg) in names.iter().enumerate() {
            assert!(
                ElemId::CLASS_NAMES[i].eq_ignore_ascii_case(reg),
                "class {i}: ElemId::CLASS_NAMES = {:?} but registry = {reg:?}",
                ElemId::CLASS_NAMES[i]
            );
            assert!(
                elems.arenas[i].class_name().eq_ignore_ascii_case(reg),
                "class {i}: arena = {:?} but registry = {reg:?}",
                elems.arenas[i].class_name()
            );
        }
    }

    /// `Idx<T>` and the `to_ref`/`class_ord` bridge round-trip a `{cls, idx}`.
    #[test]
    fn elemid_ref_bridge_round_trips() {
        // Line is class 18 in registration order (0-based).
        let names = Dss::new().registered_class_names();
        let line_cls = names
            .iter()
            .position(|n| n.eq_ignore_ascii_case("Line"))
            .unwrap();
        let id = ElemId::Line(Idx::new(7));
        assert_eq!(id.class_name(), "Line");
        assert_eq!(id.class_ord(), line_cls);
        assert_eq!(id.index(), 7);
        assert_eq!(
            id.to_ref(),
            ElemRef {
                cls: line_cls,
                idx: 7
            }
        );
    }
}
