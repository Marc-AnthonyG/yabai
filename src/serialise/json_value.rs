use serde::Serialize;
use serde_json::Value;

// serde_json::to_value widens an f32 to f64 bit for bit, so 0.9 would print as
// 0.8999999761581421; going through the text keeps the f32's own shortest spelling.
pub(crate) fn json_value_keeping_the_shortest_spelling_of_every_float<Serializable>(
    serializable: &Serializable,
) -> Value
where
    Serializable: Serialize + ?Sized,
{
    serde_json::to_string(serializable)
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::json_value_keeping_the_shortest_spelling_of_every_float;

    #[test]
    fn an_f32_keeps_the_spelling_it_was_set_with() {
        let value = json_value_keeping_the_shortest_spelling_of_every_float(&0.9f32);

        assert_eq!(value.to_string(), "0.9");
    }
}
