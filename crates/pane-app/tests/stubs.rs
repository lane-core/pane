use pane_app::{RouteTable, RouteResult, PropertyDecl, ScriptQuery, ScriptOp};

#[test]
fn route_table_returns_no_match() {
    let table = RouteTable::new();
    let result = table.route("hello.rs", "text/x-rust");
    assert!(matches!(result, RouteResult::NoMatch));
}

#[test]
fn property_decl_constructible() {
    let prop = PropertyDecl {
        name: "title".into(),
        description: "The pane title".into(),
        writable: true,
    };
    assert_eq!(prop.name, "title");
    assert!(prop.writable);
}

#[test]
fn script_query_constructible() {
    let query = ScriptQuery {
        property: "title".into(),
        operation: ScriptOp::Get,
    };
    assert_eq!(query.property, "title");
    assert!(matches!(query.operation, ScriptOp::Get));
}
