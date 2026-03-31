//! Optic law test helpers.
//!
//! Use these in property tests to verify that your optics are
//! well-behaved. Each function tests one law from Clarke et al.
//! and panics with a descriptive message on violation.

use std::fmt::Debug;

use crate::{Getter, Setter, PartialGetter, PartialSetter};

/// **GetPut law** for a lens: setting back what you got is identity.
///
/// `set(s, get(s)) == s`
///
/// Clarke et al. Def 3.1: GetPut.
pub fn assert_get_put<S, A>(
    getter: &impl Getter<S, A>,
    setter: &impl Setter<S, A>,
    source: &S,
)
where
    S: Clone + PartialEq + Debug,
    A: Clone,
{
    let mut copy = source.clone();
    let a = getter.get(source).clone();
    setter.set(&mut copy, a);
    assert_eq!(&copy, source, "GetPut violation: set(s, get(s)) != s");
}

/// **PutGet law** for a lens: you get back what you set.
///
/// `get(set(s, a)) == a`
///
/// Clarke et al. Def 3.1: PutGet.
pub fn assert_put_get<S, A>(
    getter: &impl Getter<S, A>,
    setter: &impl Setter<S, A>,
    source: &S,
    value: &A,
)
where
    S: Clone,
    A: Clone + PartialEq + Debug,
{
    let mut copy = source.clone();
    setter.set(&mut copy, value.clone());
    let got = getter.get(&copy).clone();
    assert_eq!(&got, value, "PutGet violation: get(set(s, a)) != a");
}

/// **PutPut law** for a lens: setting twice is the same as setting once.
///
/// `set(set(s, a), b) == set(s, b)`
///
/// Clarke et al. Def 3.1: PutPut.
pub fn assert_put_put<S, A>(
    setter: &impl Setter<S, A>,
    source: &S,
    value_a: &A,
    value_b: &A,
)
where
    S: Clone + PartialEq + Debug,
    A: Clone,
{
    let mut via_two = source.clone();
    setter.set(&mut via_two, value_a.clone());
    setter.set(&mut via_two, value_b.clone());

    let mut via_one = source.clone();
    setter.set(&mut via_one, value_b.clone());

    assert_eq!(via_two, via_one, "PutPut violation: set(set(s, a), b) != set(s, b)");
}

/// **Partial GetPut** for an affine: when the target exists, setting
/// back what you previewed is identity.
///
/// If `preview(s) == Some(a)`, then `set_partial(s, a) => s unchanged`.
pub fn assert_partial_get_put<S, A>(
    getter: &impl PartialGetter<S, A>,
    setter: &impl PartialSetter<S, A>,
    source: &S,
)
where
    S: Clone + PartialEq + Debug,
    A: Clone,
{
    if let Some(a) = getter.preview(source) {
        let mut copy = source.clone();
        let a = a.clone();
        let ok = setter.set_partial(&mut copy, a);
        assert!(ok, "Partial GetPut: set_partial returned false for existing target");
        assert_eq!(&copy, source, "Partial GetPut violation");
    }
    // If preview returns None, the law is trivially satisfied.
}

/// **Partial PutGet** for an affine: when set succeeds, you get back
/// what you set.
///
/// If `set_partial(s, a)` returns true, then `preview(s) == Some(a)`.
pub fn assert_partial_put_get<S, A>(
    getter: &impl PartialGetter<S, A>,
    setter: &impl PartialSetter<S, A>,
    source: &S,
    value: &A,
)
where
    S: Clone,
    A: Clone + PartialEq + Debug,
{
    let mut copy = source.clone();
    if setter.set_partial(&mut copy, value.clone()) {
        let got = getter.preview(&copy);
        assert_eq!(
            got.map(|a| a.clone()),
            Some(value.clone()),
            "Partial PutGet violation"
        );
    }
    // If set_partial returns false, the law doesn't apply.
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FieldLens;

    #[derive(Clone, Debug, PartialEq)]
    struct Point { x: i32, y: i32 }

    const X_LENS: FieldLens<Point, i32> = FieldLens {
        get: |p| &p.x,
        get_mut: |p| &mut p.x,
    };

    const Y_LENS: FieldLens<Point, i32> = FieldLens {
        get: |p| &p.y,
        get_mut: |p| &mut p.y,
    };

    #[test]
    fn field_lens_get_put() {
        let p = Point { x: 10, y: 20 };
        assert_get_put(&X_LENS, &X_LENS, &p);
        assert_get_put(&Y_LENS, &Y_LENS, &p);
    }

    #[test]
    fn field_lens_put_get() {
        let p = Point { x: 10, y: 20 };
        assert_put_get(&X_LENS, &X_LENS, &p, &42);
        assert_put_get(&Y_LENS, &Y_LENS, &p, &99);
    }

    #[test]
    fn field_lens_put_put() {
        let p = Point { x: 10, y: 20 };
        assert_put_put(&X_LENS, &p, &1, &2);
        assert_put_put(&Y_LENS, &p, &100, &200);
    }

    #[test]
    fn composed_lens_laws() {
        #[derive(Clone, Debug, PartialEq)]
        struct Line { start: Point, end: Point }

        const START: FieldLens<Line, Point> = FieldLens {
            get: |l| &l.start,
            get_mut: |l| &mut l.start,
        };

        let start_x = START.then(&X_LENS);
        let line = Line {
            start: Point { x: 1, y: 2 },
            end: Point { x: 3, y: 4 },
        };

        assert_get_put(&start_x, &start_x, &line);
        assert_put_get(&start_x, &start_x, &line, &99);
        assert_put_put(&start_x, &line, &10, &20);
    }

    #[test]
    fn affine_laws_present() {
        use crate::FieldAffine;

        #[derive(Clone, Debug, PartialEq)]
        struct MaybeVal { val: Option<i32> }

        let val_affine: FieldAffine<MaybeVal, i32> = FieldAffine {
            preview: |m| m.val.as_ref(),
            preview_mut: |m| m.val.as_mut(),
        };

        let present = MaybeVal { val: Some(42) };
        assert_partial_get_put(&val_affine, &val_affine, &present);
        assert_partial_put_get(&val_affine, &val_affine, &present, &99);
    }

    #[test]
    fn affine_laws_absent() {
        use crate::FieldAffine;

        #[derive(Clone, Debug, PartialEq)]
        struct MaybeVal { val: Option<i32> }

        let val_affine: FieldAffine<MaybeVal, i32> = FieldAffine {
            preview: |m| m.val.as_ref(),
            preview_mut: |m| m.val.as_mut(),
        };

        let absent = MaybeVal { val: None };
        // Laws are trivially satisfied when target is absent
        assert_partial_get_put(&val_affine, &val_affine, &absent);
        assert_partial_put_get(&val_affine, &val_affine, &absent, &99);
    }
}
