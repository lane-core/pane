//! Monadic lens: view is pure, set returns state + effects.
//!
//! Clarke et al. Definition 4.6. Concrete fn-pointer encoding.
//! Replaces fp_library's branded Lens for the ctl write path.

use std::fmt;

/// Effect produced by a monadic lens setter.
/// The framework executes effects after state mutation,
/// before snapshot publication.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// Send a notification to a service.
    Notify { target: &'static str, payload: String },
    /// Update the pane's body content.
    SetContent(Vec<u8>),
}

/// A named monadic lens from handler state S to focused value A.
///
/// View is pure: `fn(&S) -> A`.
/// Set mutates state and returns effects: `fn(&mut S, A) -> Vec<Effect>`.
/// Parse converts text from the ctl file: `fn(&str) -> Result<A, String>`.
///
/// The same MonadicLens serves both the read path (AttrReader
/// captures view) and the write path (AttrWriter captures
/// parse + set). No separate wiring — both are derived from
/// this single definition.
pub struct MonadicLens<S, A> {
    pub name: &'static str,
    pub view: fn(&S) -> A,
    pub set: fn(&mut S, A) -> Vec<Effect>,
    pub parse: fn(&str) -> Result<A, String>,
}

/// What operations an attribute supports.
/// Maps to FUSE permissions: ReadWrite -> 0660, ReadOnly -> 0440.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttrAccess {
    ReadWrite,
    ReadOnly,
    Computed,
}

/// Static description of a scriptable attribute.
#[derive(Debug, Clone)]
pub struct AttrInfo {
    pub name: &'static str,
    pub description: &'static str,
    pub access: AttrAccess,
    pub value_type: &'static str,
}

/// Type-erased attribute reader (read path).
/// Constructed from a MonadicLens by capturing view + Display.
pub struct AttrReader<S> {
    pub name: &'static str,
    reader: Box<dyn Fn(&S) -> String + Send + Sync>,
}

impl<S: 'static> AttrReader<S> {
    pub fn new<A: fmt::Display + 'static>(
        name: &'static str,
        view: fn(&S) -> A,
    ) -> Self {
        AttrReader {
            name,
            reader: Box::new(move |s| view(s).to_string()),
        }
    }

    pub fn from_monadic_lens<A: fmt::Display + 'static>(
        lens: &MonadicLens<S, A>,
    ) -> Self {
        let view = lens.view;
        AttrReader {
            name: lens.name,
            reader: Box::new(move |s| view(s).to_string()),
        }
    }

    pub fn read(&self, state: &S) -> String {
        (self.reader)(state)
    }
}

/// Type-erased attribute writer (write path).
/// Constructed from a MonadicLens by capturing parse + set.
pub struct AttrWriter<S> {
    pub name: &'static str,
    writer: Box<dyn Fn(&mut S, &str) -> Result<Vec<Effect>, WriteError> + Send + Sync>,
}

#[derive(Debug, PartialEq)]
pub enum WriteError {
    ParseError(String),
    ReadOnly,
}

impl<S: 'static> AttrWriter<S> {
    pub fn from_monadic_lens<A: 'static>(
        lens: &MonadicLens<S, A>,
    ) -> Self {
        let parse = lens.parse;
        let set = lens.set;
        AttrWriter {
            name: lens.name,
            writer: Box::new(move |s, text| {
                let val = parse(text).map_err(WriteError::ParseError)?;
                Ok(set(s, val))
            }),
        }
    }

    pub fn write(&self, state: &mut S, text: &str) -> Result<Vec<Effect>, WriteError> {
        (self.writer)(state, text)
    }
}

/// Collection of named attribute readers and writers.
pub struct AttrSet<S> {
    readers: Vec<AttrReader<S>>,
    writers: Vec<AttrWriter<S>>,
}

impl<S: 'static> AttrSet<S> {
    pub fn new() -> Self {
        AttrSet { readers: vec![], writers: vec![] }
    }

    pub fn register_monadic_lens<A: fmt::Display + 'static>(
        &mut self,
        lens: &MonadicLens<S, A>,
    ) {
        self.readers.push(AttrReader::from_monadic_lens(lens));
        self.writers.push(AttrWriter::from_monadic_lens(lens));
    }

    pub fn register_reader(&mut self, reader: AttrReader<S>) {
        self.readers.push(reader);
    }

    pub fn read(&self, name: &str, state: &S) -> Option<String> {
        self.readers.iter()
            .find(|r| r.name == name)
            .map(|r| r.read(state))
    }

    pub fn write(&self, name: &str, state: &mut S, text: &str) -> Result<Vec<Effect>, WriteError> {
        self.writers.iter()
            .find(|w| w.name == name)
            .ok_or_else(|| WriteError::ReadOnly)
            .and_then(|w| w.write(state, text))
    }

    pub fn names(&self) -> Vec<&'static str> {
        self.readers.iter().map(|r| r.name).collect()
    }

    /// Bulk read: all attributes from one snapshot as JSON.
    /// Values are strings (Display representation).
    pub fn to_json_str(&self, state: &S) -> String {
        let mut entries: Vec<String> = self.readers.iter()
            .map(|r| {
                let val = r.read(state);
                // Escape the value for JSON string
                let escaped = val.replace('\\', "\\\\")
                    .replace('"', "\\\"")
                    .replace('\n', "\\n");
                format!("\"{}\":\"{}\"", r.name, escaped)
            })
            .collect();
        entries.sort(); // deterministic order
        format!("{{{}}}", entries.join(","))
    }

    pub fn find_writer(&self, name: &str) -> Option<&AttrWriter<S>> {
        self.writers.iter().find(|w| w.name == name)
    }
}

/// Result of ctl dispatch.
#[derive(Debug)]
pub enum CtlResult {
    /// State mutated, no effects.
    Pure,
    /// State mutated with effects.
    WithEffects(Vec<Effect>),
    /// Lifecycle control flow.
    Lifecycle(crate::Flow),
    /// Command failed.
    Err(String),
}

/// Dispatch a ctl command through the monadic lens layer,
/// falling back to a handler method for non-optic commands.
pub fn dispatch_ctl<S: 'static>(
    attrs: &AttrSet<S>,
    state: &mut S,
    cmd: &str,
    args: &str,
    fallback: fn(&mut S, &str, &str) -> CtlResult,
) -> CtlResult {
    match attrs.find_writer(cmd) {
        Some(writer) => {
            match writer.write(state, args) {
                Ok(effects) if effects.is_empty() => CtlResult::Pure,
                Ok(effects) => CtlResult::WithEffects(effects),
                Err(e) => CtlResult::Err(format!("{:?}", e)),
            }
        }
        None => fallback(state, cmd, args),
    }
}

/// Assert all three lens laws on a MonadicLens.
/// Effects are ignored — the laws govern state.
pub fn assert_monadic_lens_laws<S, A>(
    lens: &MonadicLens<S, A>,
    s: S,
    a1: A,
    a2: A,
)
where
    S: Clone + PartialEq + fmt::Debug,
    A: Clone + PartialEq + fmt::Debug,
{
    // GetPut: set(s, get(s)) == s (full state, not just focus)
    let mut s_getput = s.clone();
    let val = (lens.view)(&s);
    let _ = (lens.set)(&mut s_getput, val);
    assert_eq!(s_getput, s, "GetPut violated for lens '{}'", lens.name);

    // PutGet: get(set(s, a)) == a
    let mut s_putget = s.clone();
    let _ = (lens.set)(&mut s_putget, a1.clone());
    assert_eq!(
        (lens.view)(&s_putget), a1,
        "PutGet violated for lens '{}'", lens.name
    );

    // PutPut: set(set(s, a1), a2) == set(s, a2)
    let mut left = s.clone();
    let _ = (lens.set)(&mut left, a1);
    let _ = (lens.set)(&mut left, a2.clone());
    let mut right = s;
    let _ = (lens.set)(&mut right, a2);
    assert_eq!(left, right, "PutPut violated for lens '{}'", lens.name);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Flow;

    // ── Shared test state ──────────────────────────────────

    #[derive(Clone, Debug, PartialEq)]
    struct EditorState {
        cursor: usize,
        buffer: String,
        dirty: bool,
        selection: Option<(usize, usize)>,
        tag: String,
        focused: bool,
    }

    impl EditorState {
        fn new() -> Self {
            EditorState {
                cursor: 0,
                buffer: "hello world".into(),
                dirty: false,
                selection: Some((2, 5)),
                tag: "Editor".into(),
                focused: false,
            }
        }
    }

    // ── Lens definitions ───────────────────────────────────

    fn cursor_lens() -> MonadicLens<EditorState, usize> {
        MonadicLens {
            name: "cursor",
            view: |s| s.cursor,
            set: |s, v| { s.cursor = v; vec![] },
            parse: |text| text.parse::<usize>().map_err(|e| e.to_string()),
        }
    }

    fn tag_lens() -> MonadicLens<EditorState, String> {
        MonadicLens {
            name: "tag",
            view: |s| s.tag.clone(),
            set: |s, v| {
                s.tag = v;
                vec![Effect::Notify {
                    target: "compositor",
                    payload: "TagChanged".into(),
                }]
            },
            parse: |text| Ok(text.to_string()),
        }
    }

    fn focus_lens() -> MonadicLens<EditorState, bool> {
        MonadicLens {
            name: "focused",
            view: |s| s.focused,
            set: |s, v| {
                s.focused = v;
                vec![Effect::Notify {
                    target: "compositor",
                    payload: format!("FocusChanged({})", v),
                }]
            },
            parse: |text| text.parse::<bool>().map_err(|e| e.to_string()),
        }
    }

    /// Compound mutation: set cursor, clear selection if cursor changed.
    /// Conditional — setting to current value is a no-op (GetPut).
    fn goto_lens() -> MonadicLens<EditorState, usize> {
        MonadicLens {
            name: "goto",
            view: |s| s.cursor,
            set: |s, v| {
                if s.cursor != v {
                    s.cursor = v;
                    s.selection = None;
                }
                vec![]
            },
            parse: |text| text.parse::<usize>().map_err(|e| e.to_string()),
        }
    }

    /// BUGGY: unconditionally clears selection. Violates GetPut.
    fn goto_lens_buggy() -> MonadicLens<EditorState, usize> {
        MonadicLens {
            name: "goto_buggy",
            view: |s| s.cursor,
            set: |s, v| {
                s.cursor = v;
                s.selection = None; // BUG: unconditional
                vec![]
            },
            parse: |text| text.parse::<usize>().map_err(|e| e.to_string()),
        }
    }

    /// BUGGY: cursor setter that flips dirty unconditionally.
    fn cursor_lens_dirty_bug() -> MonadicLens<EditorState, usize> {
        MonadicLens {
            name: "cursor_dirty",
            view: |s| s.cursor,
            set: |s, v| {
                s.cursor = v;
                s.dirty = true; // BUG: side effect on non-focused field
                vec![]
            },
            parse: |text| text.parse::<usize>().map_err(|e| e.to_string()),
        }
    }

    // ── Claim 1: MonadicLens works as a type ───────────────

    #[test]
    fn claim_1_monadic_lens_type_works() {
        let lens = cursor_lens();
        let s = EditorState::new();

        // View
        assert_eq!((lens.view)(&s), 0);

        // Set returns effects (empty for cursor)
        let mut s2 = s.clone();
        let effects = (lens.set)(&mut s2, 42);
        assert_eq!(s2.cursor, 42);
        assert!(effects.is_empty());

        // Set with effects (tag)
        let tag = tag_lens();
        let mut s3 = s.clone();
        let effects = (tag.set)(&mut s3, "New Title".into());
        assert_eq!(s3.tag, "New Title");
        assert_eq!(effects.len(), 1);

        // Parse
        assert_eq!((lens.parse)("42"), Ok(42));
        assert!((lens.parse)("not a number").is_err());
    }

    // ── Claim 2: Law harness catches violations ────────────

    #[test]
    fn claim_2_law_harness_passes_for_lawful_lens() {
        assert_monadic_lens_laws(
            &cursor_lens(),
            EditorState::new(),
            10, 20,
        );
    }

    #[test]
    #[should_panic(expected = "GetPut violated")]
    fn claim_2_law_harness_catches_getput_violation() {
        // dirty_bug setter unconditionally sets dirty=true
        assert_monadic_lens_laws(
            &cursor_lens_dirty_bug(),
            EditorState::new(), // dirty starts false
            10, 20,
        );
    }

    // ── Claim 3: Compound mutation satisfies laws ──────────

    #[test]
    fn claim_3_compound_goto_satisfies_all_laws() {
        let s = EditorState::new(); // has selection Some((2,5))
        assert_monadic_lens_laws(&goto_lens(), s, 10, 20);
    }

    // ── Claim 4: Unconditional side effects violate GetPut ─

    #[test]
    #[should_panic(expected = "GetPut violated")]
    fn claim_4_unconditional_side_effects_violate_getput() {
        let s = EditorState::new(); // has selection Some((2,5))
        // buggy goto clears selection even when cursor doesn't change
        assert_monadic_lens_laws(&goto_lens_buggy(), s, 10, 20);
    }

    // ── Claim 5: AttrWriter from MonadicLens ───────────────

    #[test]
    fn claim_5_attr_writer_from_monadic_lens() {
        let lens = cursor_lens();
        let writer = AttrWriter::from_monadic_lens(&lens);

        let mut s = EditorState::new();
        let effects = writer.write(&mut s, "42").unwrap();
        assert_eq!(s.cursor, 42);
        assert!(effects.is_empty());

        // Parse error
        let err = writer.write(&mut s, "not a number");
        assert!(matches!(err, Err(WriteError::ParseError(_))));
    }

    // ── Claim 6: Same lens serves reader and writer ────────

    #[test]
    fn claim_6_wiring_consistency_by_construction() {
        let lens = cursor_lens();
        let reader = AttrReader::from_monadic_lens(&lens);
        let writer = AttrWriter::from_monadic_lens(&lens);

        let mut s = EditorState::new();

        // Write through writer
        writer.write(&mut s, "99").unwrap();

        // Read through reader
        let val = reader.read(&s);
        assert_eq!(val, "99");

        // Both derived from the same lens — no divergence possible.
        // The view fn pointer used by reader is the same one defined
        // in the lens. The set fn pointer used by writer is also from
        // the same lens. There is no separate handler code that could
        // drift.
    }

    // ── Claim 7: Display/FromStr roundtrip ─────────────────

    #[test]
    fn claim_7_display_fromstr_roundtrip_usize() {
        for v in [0usize, 1, 42, 999, usize::MAX] {
            let s = v.to_string();
            let parsed: usize = s.parse().unwrap();
            assert_eq!(v, parsed, "roundtrip failed for {}", v);
        }
    }

    #[test]
    fn claim_7_display_fromstr_roundtrip_bool() {
        for v in [true, false] {
            let s = v.to_string();
            let parsed: bool = s.parse().unwrap();
            assert_eq!(v, parsed);
        }
    }

    #[test]
    fn claim_7_display_fromstr_roundtrip_string() {
        for v in ["", "hello", "with spaces", "special: chars!"] {
            let s = v.to_string();
            // String -> String roundtrip is identity
            assert_eq!(v, s.as_str());
        }
    }

    // ── Claim 8: AttrSet::to_json_str bulk read ────────────────

    #[test]
    fn claim_8_to_json_str_returns_all_attrs_from_one_snapshot() {
        let cursor = cursor_lens();
        let tag = tag_lens();
        let focus = focus_lens();

        let mut attrs = AttrSet::new();
        attrs.register_monadic_lens(&cursor);
        attrs.register_monadic_lens(&tag);
        attrs.register_monadic_lens(&focus);

        // Also add a read-only attr
        attrs.register_reader(AttrReader::new(
            "buffer_length",
            |s: &EditorState| s.buffer.len(),
        ));

        let s = EditorState::new();
        let json = attrs.to_json_str(&s);

        // Sorted keys for determinism
        assert_eq!(
            json,
            r#"{"buffer_length":"11","cursor":"0","focused":"false","tag":"Editor"}"#
        );

        // All values from same snapshot — cursor and buffer_length
        // are consistent because they're read from the same &S.
    }

    // ── Claim 9: Ctl dispatch routes correctly ─────────────

    #[test]
    fn claim_9_ctl_dispatch_routes_to_lens() {
        let cursor = cursor_lens();
        let tag = tag_lens();

        let mut attrs = AttrSet::new();
        attrs.register_monadic_lens(&cursor);
        attrs.register_monadic_lens(&tag);

        let mut s = EditorState::new();

        fn fallback(_: &mut EditorState, cmd: &str, _: &str) -> CtlResult {
            match cmd {
                "close" => CtlResult::Lifecycle(Flow::Stop),
                _ => CtlResult::Err(format!("unknown: {}", cmd)),
            }
        }

        // Known attr — routes through lens
        let result = dispatch_ctl(&attrs, &mut s, "cursor", "42", fallback);
        assert!(matches!(result, CtlResult::Pure));
        assert_eq!(s.cursor, 42);

        // Known attr with effects
        let result = dispatch_ctl(&attrs, &mut s, "tag", "New", fallback);
        assert!(matches!(result, CtlResult::WithEffects(_)));
        assert_eq!(s.tag, "New");

        // Unknown — falls back
        let result = dispatch_ctl(&attrs, &mut s, "close", "", fallback);
        assert!(matches!(result, CtlResult::Lifecycle(Flow::Stop)));

        // Unknown non-lifecycle — error
        let result = dispatch_ctl(&attrs, &mut s, "bogus", "", fallback);
        assert!(matches!(result, CtlResult::Err(_)));
    }

    // ── Claim 10: reload doesn't fit MonadicLens ───────────

    #[test]
    fn claim_10_reload_has_no_natural_value_type() {
        // reload reads from an external source to determine
        // the new buffer. The "value" is not provided by the
        // caller — it comes from IO. We can't write:
        //
        //   MonadicLens<EditorState, ???> {
        //       view: |s| ???,  // what do we return?
        //       set: |s, v| { s.buffer = read_from_disk(); ... },
        //       parse: |text| ???, // text is a filename, not the content
        //   }
        //
        // The set function needs IO, which monadic lens doesn't
        // provide. And view would need to return... the filename?
        // That's not a meaningful focus.
        //
        // Reload works as a freeform fallback:

        fn fallback(s: &mut EditorState, cmd: &str, args: &str) -> CtlResult {
            match cmd {
                "reload" => {
                    // In real code: s.buffer = std::fs::read_to_string(args)?;
                    s.buffer = format!("reloaded from {}", args);
                    s.dirty = false;
                    CtlResult::Pure
                }
                _ => CtlResult::Err("unknown".into()),
            }
        }

        let attrs = AttrSet::<EditorState>::new();
        let mut s = EditorState::new();
        let result = dispatch_ctl(&attrs, &mut s, "reload", "/tmp/file.txt", fallback);
        assert!(matches!(result, CtlResult::Pure));
        assert_eq!(s.buffer, "reloaded from /tmp/file.txt");
        assert!(!s.dirty);
    }

    // ── Claim 11: close violates GetPut if forced into lens ─

    #[test]
    fn claim_11_close_violates_getput() {
        // If we tried to model close as a lens on a "running" bool:
        let close_lens = MonadicLens::<EditorState, bool> {
            name: "running",
            view: |_s| true, // always "running"
            set: |_s, _v| {
                // close always terminates, regardless of value
                // GetPut: set(s, get(s)) should == s
                // But set(s, true) would still trigger shutdown
                // This is semantically wrong — close is not a setter
                vec![]
            },
            parse: |text| text.parse::<bool>().map_err(|e| e.to_string()),
        };

        // The lens itself is technically lawful (view returns true,
        // set is a no-op on state) — but it doesn't model close.
        // A "close" that actually worked would need to return
        // Flow::Stop, which is outside the lens return type.
        // The lens type literally can't express lifecycle control.

        // Prove it: the lens set doesn't return Flow, so there's
        // no way to signal "stop the looper" through this path.
        let mut s = EditorState::new();
        let effects = (close_lens.set)(&mut s, false);
        // We got effects (empty) and mutated state, but we can't
        // return Flow::Stop. The looper continues. Close didn't work.
        assert!(effects.is_empty());
        // The only way to close is through CtlResult::Lifecycle(Flow::Stop),
        // which bypasses the lens entirely.
    }

    // ── Claim 12: Projection chain faithfulness ────────────

    #[test]
    fn claim_12_projection_chain_faithful() {
        let lens = cursor_lens();
        let reader = AttrReader::from_monadic_lens(&lens);

        let s = EditorState { cursor: 42, ..EditorState::new() };

        // AttrReader output == Display of MonadicLens view
        let via_reader = reader.read(&s);
        let via_lens = (lens.view)(&s).to_string();
        assert_eq!(via_reader, via_lens);
    }

    // ── Claim 13: PutPut on full state catches dirty bugs ──

    #[test]
    fn claim_13_putput_full_state_catches_dirty_flag() {
        let lens = cursor_lens_dirty_bug();
        let s = EditorState::new(); // dirty = false

        // Weak PutPut (focused field only): passes!
        let mut left = s.clone();
        let _ = (lens.set)(&mut left, 10);
        let _ = (lens.set)(&mut left, 20);
        assert_eq!((lens.view)(&left), 20); // focused field matches

        let mut right = s.clone();
        let _ = (lens.set)(&mut right, 20);
        assert_eq!((lens.view)(&right), 20); // focused field matches

        // They agree on cursor (the focused field)
        assert_eq!((lens.view)(&left), (lens.view)(&right));

        // But full state PutPut catches the bug:
        // left: dirty was set by first set(10), stays true
        // right: dirty was set by set(20), stays true
        // In this case both end up dirty=true, so PutPut holds
        // for this specific pair. The violation shows up in GetPut
        // instead — set(s, get(s)) flips dirty from false to true.

        // GetPut is the real catch for dirty-flag bugs:
        let mut s_getput = s.clone();
        let val = (lens.view)(&s);
        let _ = (lens.set)(&mut s_getput, val);
        assert_ne!(s_getput, s, "dirty-flag bug: GetPut should show divergence");
        assert_eq!(s.dirty, false);
        assert_eq!(s_getput.dirty, true);
    }
}
