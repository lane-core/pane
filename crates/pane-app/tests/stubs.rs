use pane_app::{
    RouteTable, RouteResult, PropertyInfo, ScriptQuery, ScriptOp,
    ValueType, OpKind, SpecifierForm, Specifier,
};

#[test]
fn route_table_returns_no_match() {
    let table = RouteTable::new();
    let result = table.route("hello.rs", "text/x-rust");
    assert!(matches!(result, RouteResult::NoMatch));
}

#[test]
fn property_info_constructible() {
    let prop = PropertyInfo {
        name: "title",
        description: "The pane title",
        value_type: ValueType::String,
        operations: &[OpKind::Get, OpKind::Set],
        specifier_forms: &[SpecifierForm::Direct],
    };
    assert_eq!(prop.name, "title");
    assert_eq!(prop.value_type, ValueType::String);
}

#[test]
fn script_query_constructible() {
    let query = ScriptQuery {
        specifiers: vec![Specifier::Direct("title".into())],
        operation: ScriptOp::Get,
    };
    assert_eq!(query.specifiers[0].property(), "title");
    assert!(matches!(query.operation, ScriptOp::Get));
}
