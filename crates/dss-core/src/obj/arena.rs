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
//! An [`ElemId`] enum (one `Idx<T>` variant per class) is the typed element
//! handle the whole spine speaks (R3 replaced the pre-R3 untyped
//! `{cls, idx}` tag struct with it; R1 introduced it plus the ordering proof).
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

use crate::elements::traits::CktElement;
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
/// reports it), `ElemId`/`ClassArena` variant, concrete element type, and the
/// circuit-element tag (`ckt` if the concrete type `impl CktElement`, `data` for
/// a plain `DSS_OBJECT` general class that does not). The tag drives
/// [`ClassArena::try_ckt_elem`]'s direct `&dyn CktElement` upcast without the
/// `DssObject::as_ckt_element` `Any`-round-trip; the two must agree, pinned by
/// [`tests::arena_tag_matches_trait_ckt_view`]. The 15 `data` classes are exactly
/// the `DSS_OBJECT` ones (no `as_ckt_element` impl); the 35 `ckt` classes each
/// carry one.
///
/// Any new class must be appended here in the same position it is registered in
/// `construct.rs` — [`tests::arena_order_matches_registry`] fails otherwise.
macro_rules! with_all_classes {
    ($m:ident) => {
        $m! {
            // ── DSS_OBJECT (general / data) classes ──
            "TCC_Curve"         TccCurve          crate::elements::general::tcc_curve::TccCurveObj, data;
            "Spectrum"          Spectrum          crate::elements::general::spectrum::SpectrumObj, data;
            "LineCode"          LineCode          crate::elements::general::line_code::LineCodeObj, data;
            "GrowthShape"       GrowthShape       crate::elements::general::growth_shape::GrowthShapeObj, data;
            "XfmrCode"          XfmrCode          crate::elements::general::xfmr_code::XfmrCodeObj, data;
            "XYcurve"           XyCurve           crate::elements::general::xy_curve::XyCurveObj, data;
            "LoadShape"         LoadShape         crate::elements::general::load_shape::LoadShapeObj, data;
            "TShape"            TShape            crate::elements::general::temp_shape::TShapeObj, data;
            "PriceShape"        PriceShape        crate::elements::general::price_shape::PriceShapeObj, data;
            "WireData"          WireData          crate::elements::general::conductor_data::WireDataObj, data;
            "CNData"            CnData            crate::elements::general::conductor_data::CnDataObj, data;
            "TSData"            TsData            crate::elements::general::conductor_data::TsDataObj, data;
            "LineSpacing"       LineSpacing       crate::elements::general::line_spacing::LineSpacingObj, data;
            "LineGeometry"      LineGeometry      crate::elements::general::line_geometry::LineGeometryObj, data;
            "DynamicExp"        DynamicExp        crate::elements::general::dynamic_exp::DynamicExpObj, data;
            // ── circuit-element classes ──
            "Vsource"           Vsource           crate::elements::pc::vsource::VSource, ckt;
            "Isource"           Isource           crate::elements::pc::isource::Isource, ckt;
            "Line"              Line              crate::elements::pd::line::Line, ckt;
            "Load"              Load              crate::elements::pc::load::Load, ckt;
            "Transformer"       Transformer       crate::elements::pd::transformer::Transformer, ckt;
            "Capacitor"         Capacitor         crate::elements::pd::capacitor::Capacitor, ckt;
            "Reactor"           Reactor           crate::elements::pd::reactor::Reactor, ckt;
            "Fault"             Fault             crate::elements::pd::fault::Fault, ckt;
            "RegControl"        RegControl        crate::elements::control::reg_control::RegControl, ckt;
            "CapControl"        CapControl        crate::elements::control::cap_control::CapControl, ckt;
            "Generator"         Generator         crate::elements::pc::generator::Generator, ckt;
            "WindGen"           WindGen           crate::elements::pc::windgen::WindGen, ckt;
            "GenDispatcher"     GenDispatcher     crate::elements::control::gen_dispatcher::GenDispatcher, ckt;
            "StorageController" StorageController  crate::elements::control::storage_controller::StorageController, ckt;
            "Relay"             Relay             crate::elements::control::relay::Relay, ckt;
            "Recloser"          Recloser          crate::elements::control::recloser::Recloser, ckt;
            "Fuse"              Fuse              crate::elements::pd::fuse::Fuse, ckt;
            "SwtControl"        SwtControl        crate::elements::control::swt_control::SwtControl, ckt;
            "Storage"           Storage           crate::elements::pc::storage::Storage, ckt;
            "PVSystem"          PVSystem          crate::elements::pc::pvsystem::PVSystem, ckt;
            "UPFC"              Upfc              crate::elements::pc::upfc::Upfc, ckt;
            "UPFCControl"       UpfcControl       crate::elements::control::upfc_control::UpfcControl, ckt;
            "ESPVLControl"      EspvlControl      crate::elements::control::espvl_control::EspvlControl, ckt;
            "IndMach012"        IndMach012        crate::elements::pc::ind_mach012::IndMach012, ckt;
            "GICsource"         GicSource         crate::elements::pc::gic_source::GicSource, ckt;
            "AutoTrans"         AutoTrans         crate::elements::pd::auto_trans::AutoTrans, ckt;
            "VSConverter"       VsConverter       crate::elements::pc::vs_converter::VsConverter, ckt;
            "VCCS"              Vccs              crate::elements::pc::vccs::Vccs, ckt;
            "InvControl"        InvControl        crate::elements::control::inv_control::InvControl, ckt;
            "ExpControl"        ExpControl        crate::elements::control::exp_control::ExpControl, ckt;
            "GICLine"           GicLine           crate::elements::pc::gic_line::GicLine, ckt;
            "GICTransformer"    GicTransformer    crate::elements::pd::gic_transformer::GicTransformer, ckt;
            "Monitor"           Monitor           crate::elements::meter::monitor::Monitor, ckt;
            "EnergyMeter"       EnergyMeter       crate::elements::meter::energymeter::EnergyMeter, ckt;
            "Sensor"            Sensor            crate::elements::meter::sensor::Sensor, ckt;
        }
    };
}

/// Per-class dispatch on the [`with_all_classes!`] `ckt`/`data` tag: emit the
/// direct `&dyn CktElement` upcast for a circuit-element class, or `None` for a
/// plain `DSS_OBJECT` data class (whose concrete type does not `impl
/// CktElement`, so the cast must never be generated for it). This is what lets
/// [`ClassArena::try_ckt_elem`] replace the `DssObject::as_ckt_element`
/// `Any`-round-trip.
macro_rules! ckt_view_ref {
    (ckt, $v:ident, $idx:ident) => {
        Some(&$v[$idx] as &dyn CktElement)
    };
    (data, $v:ident, $idx:ident) => {{
        let _ = ($v.len(), $idx);
        None
    }};
}
macro_rules! ckt_view_mut {
    (ckt, $v:ident, $idx:ident) => {
        Some(&mut $v[$idx] as &mut dyn CktElement)
    };
    (data, $v:ident, $idx:ident) => {{
        let _ = ($v.len(), $idx);
        None
    }};
}

/// The one consumer macro: expands the class list into `enum ElemId`,
/// `enum ClassArena`, and their per-variant match-arm impls (`obj`/`obj_mut`/
/// `try_ckt_elem`/`try_ckt_elem_mut`/`ckt_elem`/`ckt_elem_mut`/`len`/
/// `class_name`/`push_new`/`pair_mut_same`/`triple_mut_same`/
/// `for_each_ckt_elem_mut`) plus `ElemId::CLASS_NAMES`.
macro_rules! define_arena {
    ( $( $cname:literal $variant:ident $ty:ty , $kind:ident ; )* ) => {
        /// The R2 typed element handle: one variant per registered class, each
        /// carrying a typed [`Idx<T>`]. Variant order == registration order (see
        /// module docs); [`ElemId::CLASS_NAMES`] pins it against the registry.
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
        pub enum ElemId {
            $( $variant(Idx<$ty>), )*
        }

        /// Field-less companion of [`ElemId`]: its discriminants ARE the
        /// registration ordinals (same macro, same order), so
        /// [`ElemId::class_ord`] is one constant per arm instead of a name
        /// scan. Never constructed — only `as usize`-cast.
        #[derive(Clone, Copy)]
        #[repr(usize)]
        #[allow(dead_code)]
        enum ClassOrd {
            $( $variant, )*
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

            /// The 0-based class index — the position in registration order
            /// (the value the pre-R3 `cls` tag field carried). O(1): the
            /// [`ClassOrd`] discriminant is a compile-time constant per arm.
            pub fn class_ord(self) -> usize {
                match self {
                    $( ElemId::$variant(_) => ClassOrd::$variant as usize, )*
                }
            }

            /// The 0-based object index within the class arena (the tag the
            /// pre-R3 `idx` tag field carried).
            pub fn index(self) -> usize {
                match self {
                    $( ElemId::$variant(i) => i.get(), )*
                }
            }

            /// Build the typed handle from a dynamic `(class ordinal, object
            /// index)` pair — the constructor for the registry-side producers
            /// (`find_ckt_element`, `add_ckt_element`, the class-loop reports)
            /// that discover the class by position rather than by type.
            ///
            /// O(1) via a per-variant constructor table indexed by the ordinal
            /// (registration order == variant order). Panics if `cls` is out of
            /// range — an invalid class ordinal is a construction bug, never a
            /// valid state (the pre-R3 `ElemId::new(cls, idx)` literal could not
            /// express one either, since every producer reads `cls` off the
            /// registry).
            pub fn new(cls: usize, idx: usize) -> ElemId {
                const CTORS: &[fn(usize) -> ElemId] =
                    &[ $( |i| ElemId::$variant(Idx::new(i)), )* ];
                match CTORS.get(cls) {
                    Some(ctor) => ctor(idx),
                    None => unreachable!(
                        "ElemId::new: class ordinal {cls} out of range (0..{})",
                        CTORS.len()
                    ),
                }
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

            /// Circuit-element read view of object `idx`, or `None` for a general
            /// (`DSS_OBJECT`) data class — the fallible twin of [`Self::ckt_elem`].
            /// Unlike the `DssObject::as_ckt_element` trait method this upcasts the
            /// concrete `&T` directly (`T: CktElement` for every `ckt` variant),
            /// with **no `Any` round-trip** — the arena's `ckt`/`data` tag
            /// ([`with_all_classes!`]) decides per variant. Equivalent to the trait
            /// method object-for-object ([`tests::arena_tag_matches_trait_ckt_view`]);
            /// the R2b step (d) primitive the eventual `as_ckt_element` trait-method
            /// removal builds on.
            pub fn try_ckt_elem(&self, idx: usize) -> Option<&dyn CktElement> {
                match self {
                    $( ClassArena::$variant(v) => ckt_view_ref!($kind, v, idx), )*
                }
            }

            /// Mutable [`Self::try_ckt_elem`].
            pub fn try_ckt_elem_mut(&mut self, idx: usize) -> Option<&mut dyn CktElement> {
                match self {
                    $( ClassArena::$variant(v) => ckt_view_mut!($kind, v, idx), )*
                }
            }

            /// Circuit-element read view of object `idx`; panics for a general
            /// (`DSS_OBJECT`) class, mirroring the old `ClassStore` `.expect`.
            pub fn ckt_elem(&self, idx: usize) -> &dyn CktElement {
                self.try_ckt_elem(idx)
                    .expect("ElemId must point at a circuit element")
            }

            /// Circuit-element mutable view of object `idx`.
            pub fn ckt_elem_mut(&mut self, idx: usize) -> &mut dyn CktElement {
                self.try_ckt_elem_mut(idx)
                    .expect("ElemId must point at a circuit element")
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
            /// this (same) class. The typed `clone` snapshots the source first so
            /// the target can be borrowed mutably afterwards (works even if
            /// `source == target`), then the inherent typed `make_like` copies it.
            pub fn make_like_within(&mut self, target: usize, source: usize) {
                match self {
                    $( ClassArena::$variant(v) => {
                        let src = v[source].clone();
                        v[target].make_like(&src);
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
            /// `&mut dyn CktElement`, tagged with its [`ElemId`]. General
            /// (`DSS_OBJECT`) objects are skipped. The typed `Vec<T>` is the
            /// iteration substrate `MULTITHREADING_PLAN.md` M3 later swaps to
            /// `par_iter_mut`.
            fn for_each_ckt_elem_mut(
                &mut self,
                cls: usize,
                f: &mut dyn FnMut(ElemId, &mut dyn CktElement),
            ) {
                for idx in 0..self.len() {
                    if let Some(ce) = self.try_ckt_elem_mut(idx) {
                        f(ElemId::new(cls, idx), ce);
                    }
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
    pub fn pair_mut(&mut self, a: ElemId, b: ElemId) -> (&mut dyn DssObject, &mut dyn DssObject) {
        pair_mut_arenas(&mut self.arenas, a, b)
    }

    /// Three pairwise-distinct objects borrowed mutably at once. Panics on any
    /// aliasing.
    pub fn triple_mut(
        &mut self,
        a: ElemId,
        b: ElemId,
        c: ElemId,
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
    /// [`ElemId`]. Do not funnel new bulk work through a single
    /// `&mut dyn ElemStore` entry point — use this or [`Self::arenas_mut`]
    /// (Part V thread-readiness rider).
    pub fn for_each_ckt_elem_mut(&mut self, mut f: impl FnMut(ElemId, &mut dyn CktElement)) {
        for (cls, arena) in self.arenas.iter_mut().enumerate() {
            arena.for_each_ckt_elem_mut(cls, &mut f);
        }
    }
}

/// Two pairwise-distinct objects borrowed mutably at once, over a
/// `[ClassArena]` slice (shared by [`Elements`] and the `ClassStore` view).
pub(crate) fn pair_mut_arenas(
    arenas: &mut [ClassArena],
    a: ElemId,
    b: ElemId,
) -> (&mut dyn DssObject, &mut dyn DssObject) {
    let (a_cls, a_idx) = (a.class_ord(), a.index());
    let (b_cls, b_idx) = (b.class_ord(), b.index());
    assert_ne!((a_cls, a_idx), (b_cls, b_idx), "pair_mut: aliasing refs");
    if a_cls == b_cls {
        arenas[a_cls].pair_mut_same(a_idx, b_idx)
    } else {
        let [ca, cb] = arenas
            .get_disjoint_mut([a_cls, b_cls])
            .expect("pair_mut: class index out of range");
        ca.pair_mut_cross(a_idx, cb, b_idx)
    }
}

/// Three pairwise-distinct objects borrowed mutably at once, over a
/// `[ClassArena]` slice. Splits per distinct class first, then per object
/// inside a shared class (the same case analysis as the old `ClassStore`).
pub(crate) fn triple_mut_arenas(
    arenas: &mut [ClassArena],
    a: ElemId,
    b: ElemId,
    c: ElemId,
) -> (&mut dyn DssObject, &mut dyn DssObject, &mut dyn DssObject) {
    let (a_cls, a_idx) = (a.class_ord(), a.index());
    let (b_cls, b_idx) = (b.class_ord(), b.index());
    let (c_cls, c_idx) = (c.class_ord(), c.index());
    assert!(
        (a_cls, a_idx) != (b_cls, b_idx)
            && (a_cls, a_idx) != (c_cls, c_idx)
            && (b_cls, b_idx) != (c_cls, c_idx),
        "triple_mut: aliasing refs"
    );
    if a_cls == b_cls && b_cls == c_cls {
        arenas[a_cls].triple_mut_same(a_idx, b_idx, c_idx)
    } else if a_cls == b_cls {
        let [cab, cc] = arenas
            .get_disjoint_mut([a_cls, c_cls])
            .expect("triple_mut: class index out of range");
        let (oa, ob) = cab.pair_mut_same(a_idx, b_idx);
        (oa, ob, cc.obj_mut(c_idx))
    } else if a_cls == c_cls {
        let [cac, cb] = arenas
            .get_disjoint_mut([a_cls, b_cls])
            .expect("triple_mut: class index out of range");
        let (oa, oc) = cac.pair_mut_same(a_idx, c_idx);
        (oa, cb.obj_mut(b_idx), oc)
    } else if b_cls == c_cls {
        let [ca, cbc] = arenas
            .get_disjoint_mut([a_cls, b_cls])
            .expect("triple_mut: class index out of range");
        let (ob, oc) = cbc.pair_mut_same(b_idx, c_idx);
        (ca.obj_mut(a_idx), ob, oc)
    } else {
        let [ca, cb, cc] = arenas
            .get_disjoint_mut([a_cls, b_cls, c_cls])
            .expect("triple_mut: class index out of range");
        (ca.obj_mut(a_idx), cb.obj_mut(b_idx), cc.obj_mut(c_idx))
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
        // The LIVE runtime storage: each `DssClass::arena`'s selected variant, in
        // registration order — the path that actually runs (the standalone
        // `Elements` is R2 scaffolding, so it alone would not prove the live
        // arenas are correctly ordered).
        let live = dss.live_arena_class_names();
        let elems = Elements::new();

        assert_eq!(
            ElemId::CLASS_NAMES.len(),
            names.len(),
            "ElemId covers a different class count than the registry"
        );
        assert_eq!(
            live.len(),
            names.len(),
            "live DssClass::arena count differs from the registry"
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
            // Load-bearing: the live per-`DssClass` arena variant at slot `i`.
            assert!(
                live[i].eq_ignore_ascii_case(reg),
                "class {i}: live DssClass::arena = {:?} but registry = {reg:?}",
                live[i]
            );
            assert!(
                elems.arenas[i].class_name().eq_ignore_ascii_case(reg),
                "class {i}: Elements arena = {:?} but registry = {reg:?}",
                elems.arenas[i].class_name()
            );
        }
    }

    /// The `Elements` aggregate's disjoint mutable-borrow helpers
    /// (`pair_mut`/`triple_mut`, the R2/M3 substrate) across **every**
    /// class-aliasing branch. R1 leaves `Elements` production-dead (the ownership
    /// flip lives on `DssClass::arena`); this pins the borrow case-analysis so it
    /// is verified, not "correct by inspection", before R2 adopts it. Object
    /// identity is checked by name (`push_new` stores the lowercased name).
    #[test]
    fn elements_disjoint_borrows_cover_all_branches() {
        let cls = |name: &str| {
            ElemId::CLASS_NAMES
                .iter()
                .position(|n| n.eq_ignore_ascii_case(name))
                .unwrap_or_else(|| panic!("{name} not registered"))
        };
        let line = cls("Line");
        let load = cls("Load");
        let cap = cls("Capacitor");
        let r = |cls: usize, idx: usize| ElemId::new(cls, idx);

        let mut e = Elements::new();
        assert_eq!(e.push_new(line, "l0"), 0);
        assert_eq!(e.push_new(line, "l1"), 1);
        assert_eq!(e.push_new(line, "l2"), 2);
        assert_eq!(e.push_new(load, "d0"), 0);
        assert_eq!(e.push_new(cap, "c0"), 0);

        // pair — same class.
        {
            let (a, b) = e.pair_mut(r(line, 0), r(line, 1));
            assert_eq!((a.data().name(), b.data().name()), ("l0", "l1"));
        }
        // pair — cross class.
        {
            let (a, b) = e.pair_mut(r(line, 0), r(load, 0));
            assert_eq!((a.data().name(), b.data().name()), ("l0", "d0"));
        }
        // triple — all same class.
        {
            let (a, b, c) = e.triple_mut(r(line, 0), r(line, 1), r(line, 2));
            assert_eq!(
                (a.data().name(), b.data().name(), c.data().name()),
                ("l0", "l1", "l2")
            );
        }
        // triple — a.cls == b.cls, c distinct.
        {
            let (a, b, c) = e.triple_mut(r(line, 0), r(line, 1), r(load, 0));
            assert_eq!(
                (a.data().name(), b.data().name(), c.data().name()),
                ("l0", "l1", "d0")
            );
        }
        // triple — a.cls == c.cls, b distinct.
        {
            let (a, b, c) = e.triple_mut(r(line, 0), r(load, 0), r(line, 1));
            assert_eq!(
                (a.data().name(), b.data().name(), c.data().name()),
                ("l0", "d0", "l1")
            );
        }
        // triple — b.cls == c.cls, a distinct.
        {
            let (a, b, c) = e.triple_mut(r(load, 0), r(line, 0), r(line, 1));
            assert_eq!(
                (a.data().name(), b.data().name(), c.data().name()),
                ("d0", "l0", "l1")
            );
        }
        // triple — all three classes distinct.
        {
            let (a, b, c) = e.triple_mut(r(line, 0), r(load, 0), r(cap, 0));
            assert_eq!(
                (a.data().name(), b.data().name(), c.data().name()),
                ("l0", "d0", "c0")
            );
        }
    }

    /// The aliasing guard fires (Pascal has no equivalent — this is the R2/M3
    /// safety net that a control loop never hands the same object twice).
    #[test]
    #[should_panic(expected = "aliasing refs")]
    fn elements_pair_mut_rejects_aliasing() {
        let line = ElemId::CLASS_NAMES
            .iter()
            .position(|n| n.eq_ignore_ascii_case("Line"))
            .unwrap();
        let mut e = Elements::new();
        e.push_new(line, "l0");
        let a = ElemId::new(line, 0);
        let _ = e.pair_mut(a, a);
    }

    /// `Idx<T>` and the `class_ord`/`index` tag accessors agree with the live
    /// registry, and `ElemId::new` is their inverse (the `{cls, idx}` pair the
    /// pre-R3 tag struct carried as fields).
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
        // `new` is the inverse of the `class_ord`/`index` pair — round-trip in
        // both directions.
        assert_eq!(ElemId::new(line_cls, 7), id);
        assert_eq!(ElemId::new(id.class_ord(), id.index()), id);
    }

    /// `ElemId::new` selects the correct variant for **every** registered class
    /// ordinal (the full match the spine rests on), and round-trips through
    /// `class_ord`/`index` back to the same `{cls, idx}` — checked against the
    /// live registry so any drift in the class list or registration order is
    /// caught.
    #[test]
    fn from_ref_covers_every_class_and_round_trips() {
        let names = Dss::new().registered_class_names();
        assert_eq!(names.len(), ElemId::CLASS_NAMES.len());
        for (cls, reg) in names.iter().enumerate() {
            let id = ElemId::new(cls, 3);
            assert!(
                id.class_name().eq_ignore_ascii_case(reg),
                "class {cls}: ElemId::new → {:?} but registry = {reg:?}",
                id.class_name()
            );
            assert_eq!(id.class_ord(), cls);
            assert_eq!(id.index(), 3);
            assert_eq!(
                ElemId::new(id.class_ord(), id.index()),
                id,
                "class {cls}: ElemId::new / (class_ord, index) not inverse"
            );
        }
    }

    /// The `ckt`/`data` tag drives [`ClassArena::try_ckt_elem`]'s direct upcast;
    /// it MUST agree with the `DssObject::as_ckt_element` trait method
    /// object-for-object (the equivalence that lets the tag replace the `Any`
    /// round-trip). Checked against the still-present trait method for **every**
    /// registered class, so a mis-tagged column is caught; also pins the count of
    /// circuit-element classes at 35 (the 15 `DSS_OBJECT` classes are `data`).
    #[test]
    fn arena_tag_matches_trait_ckt_view() {
        let n = ElemId::CLASS_NAMES.len();
        let mut ckt_count = 0;
        for cls in 0..n {
            let mut e = Elements::new();
            e.push_new(cls, "x");
            let arena = &mut e.arenas[cls];
            // The still-present trait method is the oracle.
            let trait_is_ckt = arena.obj(0).as_ckt_element().is_some();
            if trait_is_ckt {
                ckt_count += 1;
            }
            assert_eq!(
                arena.try_ckt_elem(0).is_some(),
                trait_is_ckt,
                "class {cls} ({}): tag try_ckt_elem disagrees with trait",
                ElemId::CLASS_NAMES[cls]
            );
            assert_eq!(
                arena.try_ckt_elem_mut(0).is_some(),
                trait_is_ckt,
                "class {cls} ({}): tag try_ckt_elem_mut disagrees with trait",
                ElemId::CLASS_NAMES[cls]
            );
        }
        assert_eq!(ckt_count, 35, "expected exactly 35 circuit-element classes");
    }
}
