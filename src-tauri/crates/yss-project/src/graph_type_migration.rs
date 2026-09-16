//! Upgrade persisted declarations at the file boundary. Live graph contracts have no
//! physical-type aliases; raw data and user-authored parameter text are never rewritten.
use crate::project_io::GraphResourceFile;
use serde::{
    Deserialize, Deserializer, Serialize, Serializer, de::Error as _, ser::SerializeStruct,
};
use serde_json::Value;

const TYPE_SCHEMA_VERSION: u32 = 1;

impl Serialize for GraphResourceFile {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut output = serializer.serialize_struct("GraphResourceFile", 5)?;
        output.serialize_field("typeSchemaVersion", &TYPE_SCHEMA_VERSION)?;
        output.serialize_field("kind", &self.kind)?;
        output.serialize_field("name", &self.name)?;
        output.serialize_field("document", &self.document)?;
        output.serialize_field("function", &self.function)?;
        output.end()
    }
}

impl<'de> Deserialize<'de> for GraphResourceFile {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Wire {
            #[serde(default)]
            type_schema_version: Option<u32>,
            kind: yss_graph_document::GraphResourceKind,
            name: String,
            document: Value,
            function: Option<Value>,
        }
        let mut wire = Wire::deserialize(deserializer)?;
        match wire.type_schema_version {
            None => upgrade_document(&mut wire.document),
            Some(TYPE_SCHEMA_VERSION) => {}
            Some(_) => return Err(D::Error::custom("unsupported graph type schema version")),
        }
        if wire.type_schema_version.is_none()
            && let Some(signature) = wire
                .function
                .as_mut()
                .and_then(|function| function.get_mut("signature"))
        {
            if let Some(parameters) = signature
                .get_mut("parameters")
                .and_then(Value::as_array_mut)
            {
                for parameter in parameters {
                    if let Some(Value::String(name)) = parameter.get_mut("type_name") {
                        *name = upgrade_name(name);
                    }
                }
            }
            if let Some(Value::String(name)) = signature.get_mut("return_type") {
                *name = upgrade_name(name);
            }
        }
        Ok(Self {
            kind: wire.kind,
            name: wire.name,
            document: serde_json::from_value(wire.document).map_err(D::Error::custom)?,
            function: wire
                .function
                .map(serde_json::from_value)
                .transpose()
                .map_err(D::Error::custom)?,
        })
    }
}

fn legacy_semantic(name: &str) -> Option<&'static str> {
    match name {
        "Boolean" => Some("Binary"),
        "Int8" | "Int16" | "Int32" | "Int64" | "UInt8" | "UInt16" | "UInt32" | "UInt64"
        | "Float32" | "Float64" | "Number" => Some("Numeric"),
        "String" => Some("Text"),
        "Date" | "Time" | "Datetime" | "DateTime" => Some("Datetime"),
        "Categorical" => Some("Categorical"),
        _ => None,
    }
}

fn upgrade_value_type(value: &mut Value) {
    if let Some(kind) = value.get("kind").and_then(Value::as_str) {
        if let Some(semantic) = legacy_semantic(kind) {
            *value = serde_json::json!({"kind": "Scalar", "inner": semantic});
            return;
        }
        match kind {
            "Array" | "DataSeries" => {
                if let Some(inner) = value.get_mut("inner") {
                    upgrade_value_type(inner);
                }
            }
            "OneOf" => {
                if let Some(members) = value.get_mut("inner").and_then(Value::as_array_mut) {
                    members.iter_mut().for_each(upgrade_value_type);
                    let mut distinct = Vec::new();
                    for member in members.iter() {
                        if !distinct.contains(member) {
                            distinct.push(member.clone());
                        }
                    }
                    if distinct.len() == 1 {
                        *value = distinct.remove(0);
                    } else {
                        *members = distinct;
                    }
                }
            }
            _ => {}
        }
    }
}

fn upgrade_expression(value: &mut Value) {
    match value {
        Value::Object(object) => {
            if let Some(Value::String(id)) = object.get_mut("Concrete") {
                let next = match id.as_str() {
                    "core.int64" | "core.float64" => "core.numeric",
                    "core.bool" => "core.binary",
                    "core.string" => "core.text",
                    "core.date" | "core.time" => "core.datetime",
                    _ => id.as_str(),
                };
                *id = next.to_owned();
            }
            object.values_mut().for_each(upgrade_expression);
        }
        Value::Array(values) => values.iter_mut().for_each(upgrade_expression),
        _ => {}
    }
}

fn upgrade_document(document: &mut Value) {
    if let Some(constants) = document.get_mut("constants").and_then(Value::as_object_mut) {
        for constant in constants.values_mut() {
            if let Some(value_type) = constant.get_mut("dataType") {
                upgrade_value_type(value_type);
            }
            if let Some(element) = constant.pointer_mut("/dataValue/DataSeries/elementType") {
                upgrade_value_type(element);
            }
        }
    }
    // These are the only persisted TypeExpr positions; node parameter values are user data.
    if let Some(states) = document
        .get_mut("input_states")
        .and_then(Value::as_array_mut)
    {
        for state in states {
            if let Some(expression) = state.pointer_mut("/1/literal_override/value_type") {
                upgrade_expression(expression);
            }
        }
    }
    if let Some(bindings) = document
        .get_mut("port_bindings")
        .and_then(Value::as_array_mut)
    {
        for binding in bindings {
            if let Some(expression) = binding.pointer_mut("/1/last_known/value_type") {
                upgrade_expression(expression);
            }
        }
    }
}

fn upgrade_name(name: &str) -> String {
    let name = name.trim();
    if let Some(semantic) = legacy_semantic(name) {
        return semantic.into();
    }
    let mut depth = 0usize;
    let mut start = 0;
    let mut members = Vec::new();
    for (index, character) in name.char_indices() {
        match character {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            '|' if depth == 0 => {
                members.push(upgrade_name(&name[start..index]));
                start = index + 1;
            }
            _ => {}
        }
    }
    if !members.is_empty() {
        members.push(upgrade_name(&name[start..]));
        let mut distinct = Vec::new();
        for member in members {
            if !distinct.contains(&member) {
                distinct.push(member);
            }
        }
        return distinct.join(" | ");
    }
    if name.starts_with("Struct<") {
        return name.into();
    }
    for container in ["Array", "DataSeries"] {
        if let Some(inner) = name
            .strip_prefix(&format!("{container}<"))
            .and_then(|value| value.strip_suffix('>'))
        {
            return format!("{container}<{}>", upgrade_name(inner));
        }
    }
    name.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn persisted_type_upgrade_preserves_values_and_function_signatures() {
        let id = "00000000-0000-0000-0000-000000000001";
        let mut document =
            serde_json::to_value(yss_graph_document::GraphDocument::default()).unwrap();
        document["constants"] = serde_json::json!({(id): {
            "id": id, "name": "raw", "dataType": {"kind": "Float64"},
            "dataValue": {"Float64": 2.5}, "description": "Float64", "tags": []
        }});
        let source = serde_json::json!({ "name": "Existing", "kind": "function", "document": document,
            "function": {"revision": 1, "signature": {"parameters": [{"id":"x", "name":"X", "type_name":"DataSeries<Float64>"}], "return_type":"Int64 | Float64"}}
        });
        let restored: GraphResourceFile = serde_json::from_value(source.clone()).unwrap();
        let current = serde_json::to_value(&restored).unwrap();
        assert_eq!(current["typeSchemaVersion"], TYPE_SCHEMA_VERSION);
        assert_eq!(
            current["document"]["constants"][id]["dataType"],
            serde_json::json!({"kind":"Scalar","inner":"Numeric"})
        );
        assert_eq!(
            current["document"]["constants"][id]["dataValue"],
            source["document"]["constants"][id]["dataValue"]
        );
        assert_eq!(
            current["document"]["constants"][id]["description"],
            "Float64"
        );
        assert_eq!(
            current["function"]["signature"]["parameters"][0]["type_name"],
            "DataSeries<Numeric>"
        );
        assert_eq!(current["function"]["signature"]["return_type"], "Numeric");
        assert_eq!(
            serde_json::to_value(
                serde_json::from_value::<GraphResourceFile>(current.clone()).unwrap()
            )
            .unwrap(),
            current
        );
        let mut invalid_version = current;
        invalid_version["typeSchemaVersion"] = Value::from(TYPE_SCHEMA_VERSION + 1);
        assert!(serde_json::from_value::<GraphResourceFile>(invalid_version).is_err());
    }
}
