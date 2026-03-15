use pane_proto::attrs::PaneMessage;
use pane_proto::server::ServerVerb;
use pane_proto::server::views::{TypedView, ViewError};
use pane_proto::server::route::{RouteCommand, RouteQuery};
use pane_proto::server::roster::{RosterRegister, RosterServiceRegister, ServerKind};
use pane_proto::{serialize, deserialize};

// --- RouteCommand ---

#[test]
fn route_command_build_parse_roundtrip() {
    let msg = RouteCommand::build()
        .data("parse.c:42")
        .wdir("/home/lane/src")
        .src("pane-comp")
        .content_type("text/plain")
        .into_message();

    assert_eq!(msg.core, ServerVerb::Command);

    let bytes = serialize(&msg).unwrap();
    let decoded: PaneMessage<ServerVerb> = deserialize(&bytes).unwrap();

    let view = RouteCommand::parse(&decoded).unwrap();
    assert_eq!(view.data, "parse.c:42");
    assert_eq!(view.wdir, "/home/lane/src");
    assert_eq!(view.src, Some("pane-comp"));
    assert_eq!(view.content_type, Some("text/plain"));
}

#[test]
fn route_command_minimal() {
    let msg = RouteCommand::build()
        .data("Makefile:10")
        .wdir("/tmp")
        .into_message();

    let view = RouteCommand::parse(&msg).unwrap();
    assert_eq!(view.data, "Makefile:10");
    assert_eq!(view.wdir, "/tmp");
    assert_eq!(view.src, None);
    assert_eq!(view.content_type, None);
}

#[test]
fn route_command_wrong_verb() {
    let msg = PaneMessage::new(ServerVerb::Query);
    let err = RouteCommand::parse(&msg).unwrap_err();
    assert!(matches!(err, ViewError::WrongVerb { expected: ServerVerb::Command, got: ServerVerb::Query }));
}

#[test]
fn route_command_missing_field() {
    let msg = PaneMessage::new(ServerVerb::Command);
    let err = RouteCommand::parse(&msg).unwrap_err();
    assert!(matches!(err, ViewError::MissingField("data")));
}

// --- RouteQuery ---

#[test]
fn route_query_build_parse_roundtrip() {
    let msg = RouteQuery::build()
        .data("https://example.com")
        .content_type("text/uri")
        .into_message();

    assert_eq!(msg.core, ServerVerb::Query);

    let bytes = serialize(&msg).unwrap();
    let decoded: PaneMessage<ServerVerb> = deserialize(&bytes).unwrap();

    let view = RouteQuery::parse(&decoded).unwrap();
    assert_eq!(view.data, "https://example.com");
    assert_eq!(view.content_type, Some("text/uri"));
}

#[test]
fn route_query_minimal() {
    let msg = RouteQuery::build()
        .data("error.log:55")
        .into_message();

    let view = RouteQuery::parse(&msg).unwrap();
    assert_eq!(view.data, "error.log:55");
    assert_eq!(view.content_type, None);
}

// --- RosterRegister ---

#[test]
fn roster_register_infrastructure() {
    let msg = RosterRegister::build()
        .signature("app.pane.route")
        .kind(ServerKind::Infrastructure)
        .socket("/run/pane/route.sock")
        .into_message();

    assert_eq!(msg.core, ServerVerb::Notify);

    let bytes = serialize(&msg).unwrap();
    let decoded: PaneMessage<ServerVerb> = deserialize(&bytes).unwrap();

    let view = RosterRegister::parse(&decoded).unwrap();
    assert_eq!(view.signature, "app.pane.route");
    assert_eq!(view.kind, ServerKind::Infrastructure);
    assert_eq!(view.socket, Some("/run/pane/route.sock"));
}

#[test]
fn roster_register_application() {
    let msg = RosterRegister::build()
        .signature("app.pane.shell")
        .kind(ServerKind::Application)
        .into_message();

    let view = RosterRegister::parse(&msg).unwrap();
    assert_eq!(view.kind, ServerKind::Application);
    assert_eq!(view.socket, None);
}

#[test]
fn roster_register_invalid_kind() {
    // Can't build an invalid kind via the builder (ServerKind enum prevents it).
    // Craft a raw message to test the parse-side validation.
    let mut msg = PaneMessage::new(ServerVerb::Notify);
    msg.set_attr("signature", pane_proto::AttrValue::String("test".into()));
    msg.set_attr("kind", pane_proto::AttrValue::String("bogus".into()));

    let err = RosterRegister::parse(&msg).unwrap_err();
    assert!(matches!(err, ViewError::InvalidValue { field: "kind", .. }));
}

#[test]
fn wrong_field_type_detected() {
    // Field present but wrong type — should be WrongFieldType, not MissingField
    let mut msg = PaneMessage::new(ServerVerb::Command);
    msg.set_attr("data", pane_proto::AttrValue::Int(42));
    msg.set_attr("wdir", pane_proto::AttrValue::String("/tmp".into()));

    let err = RouteCommand::parse(&msg).unwrap_err();
    assert!(matches!(err, ViewError::WrongFieldType { field: "data", expected: "String" }));
}

// --- RosterServiceRegister ---

#[test]
fn roster_service_register_roundtrip() {
    let msg = RosterServiceRegister::build()
        .operation("format-json")
        .content_type("application/json")
        .description("Format JSON content")
        .into_message();

    assert_eq!(msg.core, ServerVerb::Notify);

    let bytes = serialize(&msg).unwrap();
    let decoded: PaneMessage<ServerVerb> = deserialize(&bytes).unwrap();

    let view = RosterServiceRegister::parse(&decoded).unwrap();
    assert_eq!(view.operation, "format-json");
    assert_eq!(view.content_type, "application/json");
    assert_eq!(view.description, "Format JSON content");
}

#[test]
fn roster_service_register_missing_field() {
    let msg = PaneMessage::new(ServerVerb::Notify);
    let err = RosterServiceRegister::parse(&msg).unwrap_err();
    assert!(matches!(err, ViewError::MissingField("operation")));
}

// --- ViewError display ---

#[test]
fn view_error_display() {
    let e = ViewError::MissingField("data");
    assert_eq!(e.to_string(), "missing required field: data");

    let e = ViewError::WrongVerb {
        expected: ServerVerb::Command,
        got: ServerVerb::Query,
    };
    assert!(e.to_string().contains("Command"));
    assert!(e.to_string().contains("Query"));
}
