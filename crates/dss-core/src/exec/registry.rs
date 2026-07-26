//! Class-registry helper types for the command executive: the Rust
//! stand-ins for `TDSSClass` (`DssClass`), the `ElemStore` adapter
//! (`ClassStore`), and the mid-edit foreign-class view (`ForeignClasses`).
//! Split out of `exec/mod.rs`.
//!
//! DE_PASCALIZE R1 (ownership flip): the class's *objects* now live in a typed
//! per-class arena ([`ClassArena`], `DssClass::arena`) — the boxed-trait
//! `Vec<Box<dyn DssObject>>` is gone. `DssClass` still owns them (as one typed
//! `Vec<T>` per class, indexed by object position), which keeps every
//! `classes: &[DssClass]` view and the `ClassStore`/`ForeignClasses` split
//! unchanged; the objects are reached through `class.arena` instead of
//! `class.objects`.

use super::*;
use crate::obj::arena::ClassArena;

/// A class constructor: build a fresh, all-default object of the class.
pub(crate) type NewObjectFn = fn(&str) -> Box<dyn DssObject>;

/// One registered class plus its live objects — the Rust stand-in for a
/// `TDSSClass` with its `ElementNameList`. Objects live in [`Self::arena`], a
/// typed per-class `Vec<T>` behind the [`ClassArena`] enum (R1 ownership flip;
/// `PORTING_PLAN §2.1`), replacing the pre-R1 `Vec<Box<dyn DssObject>>`.
pub(crate) struct DssClass {
    pub(crate) props: ClassProps,
    pub(crate) new_object: NewObjectFn,
    /// This class's live objects — one typed `Vec<T>` (Pascal `ElementList`).
    pub(crate) arena: ClassArena,
    /// Lowercased object name → index (Pascal `ElementNameList`, THashList).
    pub(crate) name_to_idx: HashMap<String, usize>,
    /// Active object index (`ActiveElement`).
    pub(crate) active: Option<usize>,
    /// Pascal `TDSSClass.RequiresCircuit` (circuit-element classes).
    pub(crate) requires_circuit: bool,
    /// Which circuit list the elements join (`DSSObjType` class mask).
    pub(crate) kind: Option<ElemKind>,
}

impl DssClass {
    pub(crate) fn dss_object(props: ClassProps, new_object: NewObjectFn) -> Self {
        let arena = ClassArena::empty_for(props.class_name())
            .expect("every registered class has an arena variant");
        Self {
            props,
            new_object,
            arena,
            name_to_idx: HashMap::new(),
            active: None,
            requires_circuit: false,
            kind: None,
        }
    }

    pub(crate) fn ckt_class(props: ClassProps, new_object: NewObjectFn, kind: ElemKind) -> Self {
        let arena = ClassArena::empty_for(props.class_name())
            .expect("every registered class has an arena variant");
        Self {
            props,
            new_object,
            arena,
            name_to_idx: HashMap::new(),
            active: None,
            requires_circuit: true,
            kind: Some(kind),
        }
    }

    /// Pascal `SetActive`: make the named object active; returns whether it
    /// existed.
    pub(crate) fn set_active(&mut self, name: &str) -> bool {
        match self.name_to_idx.get(&name.to_ascii_lowercase()) {
            Some(&idx) => {
                self.active = Some(idx);
                true
            }
            None => false,
        }
    }
}

/// [`ElemStore`] view over the class registry — the bridge the solution
/// machinery walks instead of Pascal's pointer lists. Objects are reached
/// through each class's typed [`ClassArena`] (`classes[r.cls].arena`).
pub(crate) struct ClassStore<'a> {
    pub(crate) classes: &'a mut [DssClass],
}

impl ElemStore for ClassStore<'_> {
    fn ckt_elem(&self, r: ElemId) -> &dyn CktElement {
        self.classes[r.class_ord()].arena.ckt_elem(r.index())
    }
    fn ckt_elem_mut(&mut self, r: ElemId) -> &mut dyn CktElement {
        self.classes[r.class_ord()].arena.ckt_elem_mut(r.index())
    }

    fn obj(&self, r: ElemId) -> &dyn DssObject {
        self.classes[r.class_ord()].arena.obj(r.index())
    }

    fn kind(&self, r: ElemId) -> ElemKind {
        self.classes[r.class_ord()]
            .kind
            .expect("kind: ElemId must point at a circuit-element class")
    }

    fn find_ckt_element(&self, full_name: &str) -> Option<ElemId> {
        let lower = full_name.to_ascii_lowercase();
        let (cls_name, obj_name) = match lower.split_once('.') {
            Some((c, n)) => (Some(c), n),
            None => (None, lower.as_str()),
        };
        for (ci, class) in self.classes.iter().enumerate() {
            // Only circuit-element classes are eligible (Pascal DeviceList).
            if class.kind.is_none() {
                continue;
            }
            if let Some(cn) = cls_name
                && !class.props.class_name().eq_ignore_ascii_case(cn)
            {
                continue;
            }
            if let Some(&oi) = class.name_to_idx.get(obj_name) {
                return Some(ElemId::new(ci, oi));
            }
        }
        None
    }

    fn find_general(&self, class_name: &str, obj_name: &str) -> Option<ElemId> {
        let lower = obj_name.to_ascii_lowercase();
        for (ci, class) in self.classes.iter().enumerate() {
            if !class.props.class_name().eq_ignore_ascii_case(class_name) {
                continue;
            }
            if let Some(&oi) = class.name_to_idx.get(&lower) {
                return Some(ElemId::new(ci, oi));
            }
        }
        None
    }

    fn obj_mut(&mut self, r: ElemId) -> &mut dyn DssObject {
        self.classes[r.class_ord()].arena.obj_mut(r.index())
    }

    fn pair_mut(&mut self, a: ElemId, b: ElemId) -> (&mut dyn DssObject, &mut dyn DssObject) {
        assert_ne!(
            (a.class_ord(), a.index()),
            (b.class_ord(), b.index()),
            "pair_mut: aliasing refs"
        );
        if a.class_ord() == b.class_ord() {
            self.classes[a.class_ord()]
                .arena
                .pair_mut_same(a.index(), b.index())
        } else {
            let [ca, cb] = self
                .classes
                .get_disjoint_mut([a.class_ord(), b.class_ord()])
                .expect("pair_mut: class index out of range");
            (ca.arena.obj_mut(a.index()), cb.arena.obj_mut(b.index()))
        }
    }

    fn triple_mut(
        &mut self,
        a: ElemId,
        b: ElemId,
        c: ElemId,
    ) -> (&mut dyn DssObject, &mut dyn DssObject, &mut dyn DssObject) {
        let key = |r: ElemId| (r.class_ord(), r.index());
        assert!(
            key(a) != key(b) && key(a) != key(c) && key(b) != key(c),
            "triple_mut: aliasing refs"
        );
        if a.class_ord() == b.class_ord() && b.class_ord() == c.class_ord() {
            self.classes[a.class_ord()]
                .arena
                .triple_mut_same(a.index(), b.index(), c.index())
        } else if a.class_ord() == b.class_ord() {
            let [cab, cc] = self
                .classes
                .get_disjoint_mut([a.class_ord(), c.class_ord()])
                .expect("triple_mut: class index out of range");
            let (oa, ob) = cab.arena.pair_mut_same(a.index(), b.index());
            (oa, ob, cc.arena.obj_mut(c.index()))
        } else if a.class_ord() == c.class_ord() {
            let [cac, cb] = self
                .classes
                .get_disjoint_mut([a.class_ord(), b.class_ord()])
                .expect("triple_mut: class index out of range");
            let (oa, oc) = cac.arena.pair_mut_same(a.index(), c.index());
            (oa, cb.arena.obj_mut(b.index()), oc)
        } else if b.class_ord() == c.class_ord() {
            let [ca, cbc] = self
                .classes
                .get_disjoint_mut([a.class_ord(), b.class_ord()])
                .expect("triple_mut: class index out of range");
            let (ob, oc) = cbc.arena.pair_mut_same(b.index(), c.index());
            (ca.arena.obj_mut(a.index()), ob, oc)
        } else {
            let [ca, cb, cc] = self
                .classes
                .get_disjoint_mut([a.class_ord(), b.class_ord(), c.class_ord()])
                .expect("triple_mut: class index out of range");
            (
                ca.arena.obj_mut(a.index()),
                cb.arena.obj_mut(b.index()),
                cc.arena.obj_mut(c.index()),
            )
        }
    }
}

/// A read view of every class *except* the one being edited (the active class
/// is the excluded middle element), the [`ForeignClassesView`] the property
/// engine uses to resolve `ObjectRef` values mid-edit (PHASE4_PLAN §3.1).
/// Objects are reached through each `DssClass`'s [`ClassArena`].
pub(crate) struct ForeignClasses<'a> {
    /// `classes[..ci]` — global class index == slice index.
    pub(crate) left: &'a [DssClass],
    /// `classes[ci + 1..]` — global class index == `split + 1 + slice index`.
    pub(crate) right: &'a [DssClass],
    /// `ci`, the active class's global index.
    pub(crate) split: usize,
}

impl<'a> ForeignClasses<'a> {
    /// The first *enabled* object of a class, in creation order (Pascal's
    /// `Class.ElementList` scan in `InvControl.MakeDERList`'s empty-list branch).
    /// Returns the resolved object so callers can read its bus / phase count.
    pub(crate) fn first_enabled(&self, class: &str) -> Option<&'a dyn DssObject> {
        let scan = |c: &'a DssClass| -> Option<&'a dyn DssObject> {
            (0..c.arena.len())
                .map(|i| c.arena.obj(i))
                .find(|o| o.as_ckt_element().map(|e| e.cd().enabled).unwrap_or(false))
        };
        for c in self.left.iter().chain(self.right.iter()) {
            if c.props.class_name().eq_ignore_ascii_case(class) {
                return scan(c);
            }
        }
        None
    }

    /// The LAST *enabled* object of a class, in creation order. Pascal's
    /// InvControl/ExpControl `RecalcElementData` fleet loop assigns the
    /// control's `FNphases := ControlledElement[i].NPhases` on EVERY member, so
    /// the LAST one wins — the control's terminal shape follows the last fleet
    /// member (the `MonitoredElement`/bus stays the first).
    pub(crate) fn last_enabled(&self, class: &str) -> Option<&'a dyn DssObject> {
        let scan = |c: &'a DssClass| -> Option<&'a dyn DssObject> {
            (0..c.arena.len())
                .rev()
                .map(|i| c.arena.obj(i))
                .find(|o| o.as_ckt_element().map(|e| e.cd().enabled).unwrap_or(false))
        };
        for c in self.left.iter().chain(self.right.iter()) {
            if c.props.class_name().eq_ignore_ascii_case(class) {
                return scan(c);
            }
        }
        None
    }

    /// Resolve a (class name, object name) pair to its global [`ElemId`] plus
    /// the live object, scanning both halves. A class match with no object
    /// match short-circuits to `None`, like `cls.Find` returning NIL.
    fn lookup(&self, class: &str, name_l: &str) -> Option<(ElemId, &'a dyn DssObject)> {
        let find_in = |c: &'a DssClass, cls: usize| {
            c.name_to_idx
                .get(name_l)
                .map(|&idx| (ElemId::new(cls, idx), c.arena.obj(idx)))
        };
        let left = self.left;
        for (k, c) in left.iter().enumerate() {
            if c.props.class_name().eq_ignore_ascii_case(class) {
                return find_in(c, k);
            }
        }
        let right = self.right;
        for (k, c) in right.iter().enumerate() {
            if c.props.class_name().eq_ignore_ascii_case(class) {
                return find_in(c, self.split + 1 + k);
            }
        }
        None
    }
}

impl<'a> ForeignClassesView<'a> for ForeignClasses<'a> {
    fn find(&self, class: &str, name: &str) -> Option<(ElemId, &'a dyn DssObject)> {
        self.lookup(class, &name.to_ascii_lowercase())
    }

    /// Pascal `GetCktElementIndex`: resolve a full `Class.Name` reference (the
    /// `PropertyOffset2 = 0` object-ref case, e.g. CapControl `element=`). The
    /// returned `String` is the canonical `FullName` for dumps.
    fn find_full(&self, full_name: &str) -> Option<(ElemId, &'a dyn DssObject, String)> {
        let dot = full_name.find('.')?;
        let (class, name) = (&full_name[..dot], &full_name[dot + 1..]);
        // Reuse the per-class lookup, then rebuild the canonical FullName.
        let (r, obj) = self.lookup(class, &name.to_ascii_lowercase())?;
        let cls = if r.class_ord() < self.split {
            &self.left[r.class_ord()]
        } else {
            &self.right[r.class_ord() - self.split - 1]
        };
        Some((
            r,
            obj,
            format!("{}.{}", cls.props.class_name(), obj.data().name()),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exec::Dss;

    /// R1 Risks-section guard: a bare-name `find_ckt_element` on an object name
    /// shared across classes must still resolve to the **first-registered**
    /// class (registration-order tie-break) after the ownership flip — the arena
    /// layout preserves `construct.rs` order, so `Line` (registered before
    /// `Load`) wins over a same-named `Load`.
    #[test]
    fn find_ckt_element_tie_breaks_by_registration_order() {
        let mut dss = Dss::new();
        for c in [
            "new circuit.tie basekv=12.47 bus1=src",
            "new line.same bus1=src bus2=b",
            "new load.same bus1=b kv=12.47 kw=1",
        ] {
            dss.command(c);
        }
        assert!(dss.errors().is_empty(), "setup errors: {:?}", dss.errors());

        let line_ci = dss.class_by_name["line"];
        let load_ci = dss.class_by_name["load"];
        assert!(
            line_ci < load_ci,
            "Line must register before Load (construct.rs order)"
        );

        let store = ClassStore {
            classes: &mut dss.classes,
        };
        let r = store
            .find_ckt_element("same")
            .expect("bare name resolves to a circuit element");
        assert_eq!(
            r.class_ord(),
            line_ci,
            "bare-name find must return the first-registered class (Line), not Load"
        );
    }
}
