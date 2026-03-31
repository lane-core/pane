//! Composable optic types for structured state access.
//!
//! Pure optic types with no pane-specific dependencies. The theory
//! (profunctor optics, optic laws) makes the system sound; consumers
//! of this crate see simple get/set traits and concrete accessor structs.
//!
//! # Optic families
//!
//! ```text
//! Traversal (zero-or-more) ⊃ Affine (zero-or-one) ⊃ Lens (exactly-one)
//! ```
//!
//! **Affine is the default** for scripting — targets may not exist
//! (closed pane, empty selection, missing attribute). Lenses are for
//! fields that structurally must be present.
//!
//! # Two layers
//!
//! - **Concrete optics** (`FieldLens`, `FieldAffine`, `FieldTraversal`):
//!   zero-cost, monomorphic, reference-returning. Used within handlers
//!   where all types are known.
//! - **Trait optics** (`Getter`, `PartialGetter`, etc.): dyn-compatible
//!   traits for composition and dynamic dispatch at protocol boundaries.
//!
//! # Composition
//!
//! All optic types compose via `.then()`. Composition follows the
//! optic subtyping lattice — composing a lens with an affine produces
//! an affine, etc.
//!
//! # Optic laws
//!
//! The `laws` module provides test helpers that verify GetPut, PutGet,
//! and PutPut for any concrete optic. Use these in property tests to
//! ensure your optics are well-behaved.

pub mod compose;
pub mod laws;

use std::fmt;

// ---------------------------------------------------------------------------
// Core traits — reference-returning, dyn-compatible
// ---------------------------------------------------------------------------

/// Total getter — the target always exists.
///
/// Produces `&A` from `&S`. This is the "get" half of a lens.
/// dyn-compatible: the lifetime `'s` ties the reference to the source.
pub trait Getter<S, A: ?Sized> {
    fn get<'s>(&self, source: &'s S) -> &'s A;
}

/// Total mutable access — the target always exists.
///
/// Produces `&mut A` from `&mut S`. Used internally by `FieldLens`
/// for the "set via mutable reference" pattern.
pub trait GetterMut<S, A: ?Sized> {
    fn get_mut<'s>(&self, source: &'s mut S) -> &'s mut A;
}

/// Total setter — write a value into the target.
///
/// This is the "set" half of a lens. Always succeeds because the
/// target is guaranteed to exist (it's a lens, not an affine).
pub trait Setter<S, A> {
    fn set(&self, source: &mut S, value: A);
}

/// Partial getter — the target may not exist.
///
/// Returns `Option<&A>`. This is the "preview" half of an affine
/// optic. `None` means the target is absent (window closed,
/// selection empty, index out of range).
pub trait PartialGetter<S, A: ?Sized> {
    fn preview<'s>(&self, source: &'s S) -> Option<&'s A>;
}

/// Partial setter — set may fail if target absent.
///
/// Returns `true` if the set succeeded, `false` if the target
/// was absent. This is the "set" half of an affine optic.
pub trait PartialSetter<S, A> {
    fn set_partial(&self, source: &mut S, value: A) -> bool;
}

// ---------------------------------------------------------------------------
// Concrete structs — zero-cost, monomorphic
// ---------------------------------------------------------------------------

/// A field lens from function pointers. Zero-cost, monomorphic.
///
/// The get function returns a reference; the get_mut function returns
/// a mutable reference (used for set: `*get_mut(s) = value`).
/// This is the Druid `Field` pattern adapted for pane.
///
/// # Laws (Clarke et al. Def 3.1)
///
/// - **GetPut:** `set(s, get(s)) == s`
/// - **PutGet:** `get(set(s, a)) == a`
/// - **PutPut:** `set(set(s, a), b) == set(s, b)`
///
/// Use [`laws::assert_get_put`] and [`laws::assert_put_get`] to test.
pub struct FieldLens<S, A> {
    pub get: fn(&S) -> &A,
    pub get_mut: fn(&mut S) -> &mut A,
}

impl<S, A> Getter<S, A> for FieldLens<S, A> {
    fn get<'s>(&self, source: &'s S) -> &'s A {
        (self.get)(source)
    }
}

impl<S, A> GetterMut<S, A> for FieldLens<S, A> {
    fn get_mut<'s>(&self, source: &'s mut S) -> &'s mut A {
        (self.get_mut)(source)
    }
}

impl<S, A> Setter<S, A> for FieldLens<S, A> {
    fn set(&self, source: &mut S, value: A) {
        *(self.get_mut)(source) = value;
    }
}

// A lens is also an affine (total ⊂ partial).
impl<S, A> PartialGetter<S, A> for FieldLens<S, A> {
    fn preview<'s>(&self, source: &'s S) -> Option<&'s A> {
        Some((self.get)(source))
    }
}

impl<S, A> PartialSetter<S, A> for FieldLens<S, A> {
    fn set_partial(&self, source: &mut S, value: A) -> bool {
        *(self.get_mut)(source) = value;
        true
    }
}

impl<S, A> Clone for FieldLens<S, A> {
    fn clone(&self) -> Self { *self }
}
impl<S, A> Copy for FieldLens<S, A> {}

impl<S, A> fmt::Debug for FieldLens<S, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FieldLens").finish()
    }
}

/// An affine accessor from function pointers. Partial get/set.
///
/// The target may or may not exist. `preview` returns `None` when
/// absent. `set_partial` returns `false` when absent.
///
/// # Laws (Clarke et al. Def 3.7, weakened to Option)
///
/// - **GetPut:** `preview(s).map(|a| set(s, a)) ≡ Some(s)` when present
/// - **PutGet:** `set(s, a) => preview(s) == Some(a)` when set succeeds
pub struct FieldAffine<S, A> {
    pub preview: fn(&S) -> Option<&A>,
    pub preview_mut: fn(&mut S) -> Option<&mut A>,
}

impl<S, A> PartialGetter<S, A> for FieldAffine<S, A> {
    fn preview<'s>(&self, source: &'s S) -> Option<&'s A> {
        (self.preview)(source)
    }
}

impl<S, A> PartialSetter<S, A> for FieldAffine<S, A> {
    fn set_partial(&self, source: &mut S, value: A) -> bool {
        match (self.preview_mut)(source) {
            Some(target) => {
                *target = value;
                true
            }
            None => false,
        }
    }
}

impl<S, A> Clone for FieldAffine<S, A> {
    fn clone(&self) -> Self { *self }
}
impl<S, A> Copy for FieldAffine<S, A> {}

impl<S, A> fmt::Debug for FieldAffine<S, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FieldAffine").finish()
    }
}

/// A traversal — zero-or-more targets in a collection.
///
/// `contents` collects all targets. `modify` applies a function
/// to each target in place. For indexed access, use `at(i)` to
/// produce an affine focused on a single element.
pub struct FieldTraversal<S, A> {
    pub contents: fn(&S) -> Vec<&A>,
    pub modify: fn(&mut S, &dyn Fn(&A) -> A),
}

impl<S, A> FieldTraversal<S, A> {
    /// Count the number of targets.
    pub fn count(&self, source: &S) -> usize {
        (self.contents)(source).len()
    }
}

impl<S, A> Clone for FieldTraversal<S, A> {
    fn clone(&self) -> Self { *self }
}
impl<S, A> Copy for FieldTraversal<S, A> {}

impl<S, A> fmt::Debug for FieldTraversal<S, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FieldTraversal").finish()
    }
}
