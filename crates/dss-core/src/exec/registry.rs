//! Class-registry helper types for the command executive: the Rust
//! stand-ins for `TDSSClass` (`DssClass`), the `ElemStore` adapter
//! (`ClassStore`), and the mid-edit foreign-class view (`ForeignClasses`).
//! Split out of `exec/mod.rs`.

use super::*;

/// A class constructor: build a fresh, all-default object of the class.
pub(crate) type NewObjectFn = fn(&str) -> Box<dyn DssObject>;

/// One registered class plus its live objects — the Rust stand-in for a
/// `TDSSClass` together with its `ElementList`/`ElementNameList`.
pub(crate) struct DssClass {
    pub(crate) props: ClassProps,
    pub(crate) new_object: NewObjectFn,
    pub(crate) objects: Vec<Box<dyn DssObject>>,
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
        Self {
            props,
            new_object,
            objects: Vec::new(),
            name_to_idx: HashMap::new(),
            active: None,
            requires_circuit: false,
            kind: None,
        }
    }

    pub(crate) fn ckt_class(props: ClassProps, new_object: NewObjectFn, kind: ElemKind) -> Self {
        Self {
            props,
            new_object,
            objects: Vec::new(),
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
/// machinery walks instead of Pascal's pointer lists.
pub(crate) struct ClassStore<'a> {
    pub(crate) classes: &'a mut [DssClass],
}

impl ElemStore for ClassStore<'_> {
    fn ckt_elem(&self, r: ElemRef) -> &dyn CktElement {
        self.classes[r.cls].objects[r.idx]
            .as_ckt_element()
            .expect("ElemRef must point at a circuit element")
    }
    fn ckt_elem_mut(&mut self, r: ElemRef) -> &mut dyn CktElement {
        self.classes[r.cls].objects[r.idx]
            .as_ckt_element_mut()
            .expect("ElemRef must point at a circuit element")
    }

    fn obj(&self, r: ElemRef) -> &dyn DssObject {
        self.classes[r.cls].objects[r.idx].as_ref()
    }

    fn kind(&self, r: ElemRef) -> ElemKind {
        self.classes[r.cls]
            .kind
            .expect("kind: ElemRef must point at a circuit-element class")
    }

    fn find_ckt_element(&self, full_name: &str) -> Option<ElemRef> {
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
                return Some(ElemRef { cls: ci, idx: oi });
            }
        }
        None
    }

    fn find_general(&self, class_name: &str, obj_name: &str) -> Option<ElemRef> {
        let lower = obj_name.to_ascii_lowercase();
        for (ci, class) in self.classes.iter().enumerate() {
            if !class.props.class_name().eq_ignore_ascii_case(class_name) {
                continue;
            }
            if let Some(&oi) = class.name_to_idx.get(&lower) {
                return Some(ElemRef { cls: ci, idx: oi });
            }
        }
        None
    }

    fn obj_mut(&mut self, r: ElemRef) -> &mut dyn DssObject {
        self.classes[r.cls].objects[r.idx].as_mut()
    }

    fn pair_mut(&mut self, a: ElemRef, b: ElemRef) -> (&mut dyn DssObject, &mut dyn DssObject) {
        assert_ne!((a.cls, a.idx), (b.cls, b.idx), "pair_mut: aliasing refs");
        if a.cls == b.cls {
            let objs = &mut self.classes[a.cls].objects;
            let [oa, ob] = objs
                .get_disjoint_mut([a.idx, b.idx])
                .expect("pair_mut: object index out of range");
            (oa.as_mut(), ob.as_mut())
        } else {
            let [ca, cb] = self
                .classes
                .get_disjoint_mut([a.cls, b.cls])
                .expect("pair_mut: class index out of range");
            (ca.objects[a.idx].as_mut(), cb.objects[b.idx].as_mut())
        }
    }

    fn triple_mut(
        &mut self,
        a: ElemRef,
        b: ElemRef,
        c: ElemRef,
    ) -> (&mut dyn DssObject, &mut dyn DssObject, &mut dyn DssObject) {
        let key = |r: ElemRef| (r.cls, r.idx);
        assert!(
            key(a) != key(b) && key(a) != key(c) && key(b) != key(c),
            "triple_mut: aliasing refs"
        );
        // Split per distinct class first, then per object inside a shared class.
        if a.cls == b.cls && b.cls == c.cls {
            let objs = &mut self.classes[a.cls].objects;
            let [oa, ob, oc] = objs
                .get_disjoint_mut([a.idx, b.idx, c.idx])
                .expect("triple_mut: object index out of range");
            (oa.as_mut(), ob.as_mut(), oc.as_mut())
        } else if a.cls == b.cls {
            let [cab, cc] = self
                .classes
                .get_disjoint_mut([a.cls, c.cls])
                .expect("triple_mut: class index out of range");
            let [oa, ob] = cab
                .objects
                .get_disjoint_mut([a.idx, b.idx])
                .expect("triple_mut: object index out of range");
            (oa.as_mut(), ob.as_mut(), cc.objects[c.idx].as_mut())
        } else if a.cls == c.cls {
            let [cac, cb] = self
                .classes
                .get_disjoint_mut([a.cls, b.cls])
                .expect("triple_mut: class index out of range");
            let [oa, oc] = cac
                .objects
                .get_disjoint_mut([a.idx, c.idx])
                .expect("triple_mut: object index out of range");
            (oa.as_mut(), cb.objects[b.idx].as_mut(), oc.as_mut())
        } else if b.cls == c.cls {
            let [ca, cbc] = self
                .classes
                .get_disjoint_mut([a.cls, b.cls])
                .expect("triple_mut: class index out of range");
            let [ob, oc] = cbc
                .objects
                .get_disjoint_mut([b.idx, c.idx])
                .expect("triple_mut: object index out of range");
            (ca.objects[a.idx].as_mut(), ob.as_mut(), oc.as_mut())
        } else {
            let [ca, cb, cc] = self
                .classes
                .get_disjoint_mut([a.cls, b.cls, c.cls])
                .expect("triple_mut: class index out of range");
            (
                ca.objects[a.idx].as_mut(),
                cb.objects[b.idx].as_mut(),
                cc.objects[c.idx].as_mut(),
            )
        }
    }
}

/// A read view of every class *except* the one being edited (the active class
/// is the excluded middle element), the [`ForeignClassesView`] the property
/// engine uses to resolve `ObjectRef` values mid-edit (PHASE4_PLAN §3.1).
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
            c.objects
                .iter()
                .map(|o| o.as_ref())
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
            c.objects
                .iter()
                .rev()
                .map(|o| o.as_ref())
                .find(|o| o.as_ckt_element().map(|e| e.cd().enabled).unwrap_or(false))
        };
        for c in self.left.iter().chain(self.right.iter()) {
            if c.props.class_name().eq_ignore_ascii_case(class) {
                return scan(c);
            }
        }
        None
    }

    /// Resolve a (class name, object name) pair to its global [`ElemRef`] plus
    /// the live object, scanning both halves. A class match with no object
    /// match short-circuits to `None`, like `cls.Find` returning NIL.
    fn lookup(&self, class: &str, name_l: &str) -> Option<(ElemRef, &'a dyn DssObject)> {
        let find_in = |c: &'a DssClass, cls: usize| {
            c.name_to_idx
                .get(name_l)
                .map(|&idx| (ElemRef { cls, idx }, c.objects[idx].as_ref()))
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
    fn find(&self, class: &str, name: &str) -> Option<(ElemRef, &'a dyn DssObject)> {
        self.lookup(class, &name.to_ascii_lowercase())
    }

    /// Pascal `GetCktElementIndex`: resolve a full `Class.Name` reference (the
    /// `PropertyOffset2 = 0` object-ref case, e.g. CapControl `element=`). The
    /// returned `String` is the canonical `FullName` for dumps.
    fn find_full(&self, full_name: &str) -> Option<(ElemRef, &'a dyn DssObject, String)> {
        let dot = full_name.find('.')?;
        let (class, name) = (&full_name[..dot], &full_name[dot + 1..]);
        // Reuse the per-class lookup, then rebuild the canonical FullName.
        let (r, obj) = self.lookup(class, &name.to_ascii_lowercase())?;
        let cls = if r.cls < self.split {
            &self.left[r.cls]
        } else {
            &self.right[r.cls - self.split - 1]
        };
        Some((
            r,
            obj,
            format!("{}.{}", cls.props.class_name(), obj.data().name()),
        ))
    }
}
