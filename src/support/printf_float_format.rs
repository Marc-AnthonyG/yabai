pub fn format_float_with_decimals_as_printf_does(value: f64, decimals: usize) -> String {
    if value.is_nan() {
        return "nan".to_string();
    }
    format!("{:.*}", decimals, value)
}
