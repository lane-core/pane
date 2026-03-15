## 1. TypedView trait and ViewError

- [x] 1.1 Define `ViewError` enum: WrongVerb, MissingField, WrongFieldType, InvalidValue — with Display impl
- [x] 1.2 Define `TypedView` trait: `fn parse(msg: &PaneMessage<ServerVerb>) -> Result<Self, ViewError>`
- [x] 1.3 Define typestate marker types `Set` and `Unset` for builders
- [x] 1.4 Restructure `server.rs` into `server/mod.rs` with sub-modules: `views`, `route`, `roster`
- [x] 1.5 Verify `cargo build && cargo test`

## 2. RouteCommand view and builder

- [x] 2.1 Define `RouteCommand` struct with borrowed fields: data, wdir, src (optional), content_type (optional)
- [x] 2.2 Implement `TypedView::parse` for `RouteCommand` — validates verb is Command, extracts required/optional fields
- [x] 2.3 Define `RouteCommandBuilder` with typestate for data (required) and wdir (required)
- [x] 2.4 Implement `into_message()` on fully-set builder
- [x] 2.5 Test: build → into_message → serialize → deserialize → parse → assert fields match
- [x] 2.6 Verify `cargo build && cargo test`

## 3. RouteQuery view and builder

- [x] 3.1 Define `RouteQuery` struct with borrowed fields: data, content_type (optional)
- [x] 3.2 Implement `TypedView::parse` for `RouteQuery` — validates verb is Query
- [x] 3.3 Define `RouteQueryBuilder` with typestate for data (required)
- [x] 3.4 Test: build → serialize → deserialize → parse round-trip
- [x] 3.5 Verify `cargo build && cargo test`

## 4. RosterRegister view and builder

- [x] 4.1 Define `RosterRegister` struct: signature, kind, socket (optional)
- [x] 4.2 Implement `TypedView::parse` — validates verb is Notify
- [x] 4.3 Define `RosterRegisterBuilder` with typestate for signature and kind
- [x] 4.4 Test: build → serialize → deserialize → parse round-trip
- [x] 4.5 Verify `cargo build && cargo test`

## 5. RosterServiceRegister view and builder

- [x] 5.1 Define `RosterServiceRegister` struct: operation, content_type, description
- [x] 5.2 Implement `TypedView::parse` — validates verb is Notify
- [x] 5.3 Define `RosterServiceRegisterBuilder` with typestate for all three required fields
- [x] 5.4 Test: build → serialize → deserialize → parse round-trip
- [x] 5.5 Verify `cargo build && cargo test`
