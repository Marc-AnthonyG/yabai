use serde::Serialize;
use serde_json::Value;

pub(crate) fn pretty_json_keeping_only_the_selected_fields<Snapshot, FieldName>(
    snapshot_or_snapshots: &Snapshot,
    selected_fields: &[FieldName],
) -> String
where
    Snapshot: Serialize + ?Sized,
    FieldName: Serialize,
{
    let mut json = serde_json::to_value(snapshot_or_snapshots).unwrap_or_default();
    if !selected_fields.is_empty() {
        let keys_of_the_selected_fields = keys_of_fields(selected_fields);
        match &mut json {
            Value::Array(snapshots) => {
                for snapshot in snapshots {
                    keep_only_the_keys(snapshot, &keys_of_the_selected_fields);
                }
            }
            snapshot => keep_only_the_keys(snapshot, &keys_of_the_selected_fields),
        }
    }
    serde_json::to_string_pretty(&json).unwrap_or_default()
}

fn keys_of_fields<FieldName: Serialize>(fields: &[FieldName]) -> Vec<String> {
    fields
        .iter()
        .filter_map(|field| match serde_json::to_value(field) {
            Ok(Value::String(key)) => Some(key),
            _ => None,
        })
        .collect()
}

fn keep_only_the_keys(snapshot: &mut Value, keys_to_keep: &[String]) {
    if let Value::Object(fields) = snapshot {
        fields.retain(|key, _| keys_to_keep.contains(key));
    }
}

#[cfg(test)]
mod tests {
    use serde::Serialize;

    use super::pretty_json_keeping_only_the_selected_fields;
    use crate::command::query::WindowFieldName;

    #[derive(Serialize)]
    struct Snapshot {
        id: u32,
        app: &'static str,
        is_minimized: bool,
    }

    fn snapshot_of(id: u32) -> Snapshot {
        Snapshot {
            id,
            app: "Finder",
            is_minimized: false,
        }
    }

    #[test]
    fn without_selected_fields_every_field_prints_in_declaration_order() {
        let json = pretty_json_keeping_only_the_selected_fields::<_, WindowFieldName>(
            &snapshot_of(7),
            &[],
        );

        assert_eq!(
            json,
            "{\n  \"id\": 7,\n  \"app\": \"Finder\",\n  \"is_minimized\": false\n}"
        );
    }

    #[test]
    fn selected_fields_keep_declaration_order_whatever_order_they_were_given_in() {
        let json = pretty_json_keeping_only_the_selected_fields(
            &snapshot_of(7),
            &[WindowFieldName::IsMinimized, WindowFieldName::Id],
        );

        assert_eq!(json, "{\n  \"id\": 7,\n  \"is_minimized\": false\n}");
    }

    #[test]
    fn selected_fields_apply_to_every_snapshot_of_an_array() {
        let json = pretty_json_keeping_only_the_selected_fields(
            &[snapshot_of(1), snapshot_of(2)],
            &[WindowFieldName::Id],
        );

        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&json).unwrap(),
            serde_json::json!([{ "id": 1 }, { "id": 2 }])
        );
    }

    #[test]
    fn an_empty_array_prints_as_an_empty_array() {
        let json = pretty_json_keeping_only_the_selected_fields::<[Snapshot], WindowFieldName>(
            &[],
            &[WindowFieldName::Id],
        );

        assert_eq!(json, "[]");
    }
}
