fn value_is_non_empty_array(value: Option<&Value>) -> bool {
    value
        .and_then(Value::as_array)
        .is_some_and(|items| !items.is_empty())
}

fn value_is_non_empty_object(value: Option<&Value>) -> bool {
    value
        .and_then(Value::as_object)
        .is_some_and(|items| !items.is_empty())
}
