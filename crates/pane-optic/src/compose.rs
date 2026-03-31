//! Optic composition — the `.then()` combinator.
//!
//! Composition follows the optic subtyping lattice:
//! - Lens + Lens → Lens
//! - Lens + Affine → Affine
//! - Affine + Lens → Affine
//! - Affine + Affine → Affine
//!
//! This is Clarke et al. Proposition 2.3: optics form a category
//! under composition, and composed optics preserve their laws.

use std::marker::PhantomData;

use crate::{
    FieldLens, FieldAffine,
    Getter, GetterMut, Setter, PartialGetter, PartialSetter,
};

/// Composed lens: navigate through `outer` then `inner`.
///
/// Both outer and inner must be total (lenses). The result is a lens.
#[derive(Clone, Copy, Debug)]
pub struct ThenLens<S, M, A> {
    outer_get: fn(&S) -> &M,
    outer_get_mut: fn(&mut S) -> &mut M,
    inner_get: fn(&M) -> &A,
    inner_get_mut: fn(&mut M) -> &mut A,
    _phantom: PhantomData<(S, M, A)>,
}

impl<S, M: 'static, A> Getter<S, A> for ThenLens<S, M, A> {
    fn get<'s>(&self, source: &'s S) -> &'s A {
        (self.inner_get)((self.outer_get)(source))
    }
}

impl<S, M: 'static, A> GetterMut<S, A> for ThenLens<S, M, A> {
    fn get_mut<'s>(&self, source: &'s mut S) -> &'s mut A {
        (self.inner_get_mut)((self.outer_get_mut)(source))
    }
}

impl<S, M: 'static, A> Setter<S, A> for ThenLens<S, M, A> {
    fn set(&self, source: &mut S, value: A) {
        *(self.inner_get_mut)((self.outer_get_mut)(source)) = value;
    }
}

impl<S, M: 'static> FieldLens<S, M> {
    /// Compose with an inner lens, producing a composed lens.
    ///
    /// `outer.then(inner)` navigates through `outer` first, then `inner`.
    /// The result is a lens (total get, total set).
    pub fn then<B>(&self, inner: &FieldLens<M, B>) -> ThenLens<S, M, B> {
        ThenLens {
            outer_get: self.get,
            outer_get_mut: self.get_mut,
            inner_get: inner.get,
            inner_get_mut: inner.get_mut,
            _phantom: PhantomData,
        }
    }

    /// Compose a lens with an inner affine, producing an affine.
    ///
    /// The outer lens is total; the inner affine is partial. The
    /// result is partial (affine).
    pub fn then_affine<B>(&self, inner: &FieldAffine<M, B>) -> ThenLensAffine<S, M, B> {
        ThenLensAffine {
            outer_get: self.get,
            outer_get_mut: self.get_mut,
            inner_preview: inner.preview,
            inner_preview_mut: inner.preview_mut,
            _phantom: PhantomData,
        }
    }
}

/// Composed lens-then-affine: outer is total, inner is partial.
/// Result is an affine.
#[derive(Clone, Copy, Debug)]
pub struct ThenLensAffine<S, M, A> {
    outer_get: fn(&S) -> &M,
    outer_get_mut: fn(&mut S) -> &mut M,
    inner_preview: fn(&M) -> Option<&A>,
    inner_preview_mut: fn(&mut M) -> Option<&mut A>,
    _phantom: PhantomData<(S, M, A)>,
}

impl<S, M: 'static, A> PartialGetter<S, A> for ThenLensAffine<S, M, A> {
    fn preview<'s>(&self, source: &'s S) -> Option<&'s A> {
        (self.inner_preview)((self.outer_get)(source))
    }
}

impl<S, M: 'static, A> PartialSetter<S, A> for ThenLensAffine<S, M, A> {
    fn set_partial(&self, source: &mut S, value: A) -> bool {
        match (self.inner_preview_mut)((self.outer_get_mut)(source)) {
            Some(target) => {
                *target = value;
                true
            }
            None => false,
        }
    }
}

impl<S, M: 'static> FieldAffine<S, M> {
    /// Compose an affine with an inner lens, producing an affine.
    pub fn then_lens<B>(&self, inner: &FieldLens<M, B>) -> ThenAffineLens<S, M, B> {
        ThenAffineLens {
            outer_preview: self.preview,
            outer_preview_mut: self.preview_mut,
            inner_get: inner.get,
            inner_get_mut: inner.get_mut,
            _phantom: PhantomData,
        }
    }

    /// Compose an affine with an inner affine, producing an affine.
    pub fn then_affine<B>(&self, inner: &FieldAffine<M, B>) -> ThenAffineAffine<S, M, B> {
        ThenAffineAffine {
            outer_preview: self.preview,
            outer_preview_mut: self.preview_mut,
            inner_preview: inner.preview,
            inner_preview_mut: inner.preview_mut,
            _phantom: PhantomData,
        }
    }
}

/// Composed affine-then-lens.
#[derive(Clone, Copy, Debug)]
pub struct ThenAffineLens<S, M, A> {
    outer_preview: fn(&S) -> Option<&M>,
    outer_preview_mut: fn(&mut S) -> Option<&mut M>,
    inner_get: fn(&M) -> &A,
    inner_get_mut: fn(&mut M) -> &mut A,
    _phantom: PhantomData<(S, M, A)>,
}

impl<S, M: 'static, A> PartialGetter<S, A> for ThenAffineLens<S, M, A> {
    fn preview<'s>(&self, source: &'s S) -> Option<&'s A> {
        (self.outer_preview)(source).map(|m| (self.inner_get)(m))
    }
}

impl<S, M: 'static, A> PartialSetter<S, A> for ThenAffineLens<S, M, A> {
    fn set_partial(&self, source: &mut S, value: A) -> bool {
        match (self.outer_preview_mut)(source) {
            Some(m) => {
                *(self.inner_get_mut)(m) = value;
                true
            }
            None => false,
        }
    }
}

/// Composed affine-then-affine.
#[derive(Clone, Copy, Debug)]
pub struct ThenAffineAffine<S, M, A> {
    outer_preview: fn(&S) -> Option<&M>,
    outer_preview_mut: fn(&mut S) -> Option<&mut M>,
    inner_preview: fn(&M) -> Option<&A>,
    inner_preview_mut: fn(&mut M) -> Option<&mut A>,
    _phantom: PhantomData<(S, M, A)>,
}

impl<S, M: 'static, A> PartialGetter<S, A> for ThenAffineAffine<S, M, A> {
    fn preview<'s>(&self, source: &'s S) -> Option<&'s A> {
        (self.outer_preview)(source).and_then(|m| (self.inner_preview)(m))
    }
}

impl<S, M: 'static, A> PartialSetter<S, A> for ThenAffineAffine<S, M, A> {
    fn set_partial(&self, source: &mut S, value: A) -> bool {
        match (self.outer_preview_mut)(source) {
            Some(m) => match (self.inner_preview_mut)(m) {
                Some(target) => {
                    *target = value;
                    true
                }
                None => false,
            },
            None => false,
        }
    }
}
