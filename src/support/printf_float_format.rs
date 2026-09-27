pub fn format_float_with_decimals_as_printf_does(value: f64, decimals: usize) -> String {
    if value.is_nan() {
        return "nan".to_string();
    }
    format!("{:.*}", decimals, value)
}

#[cfg(test)]
mod tests {
    use super::format_float_with_decimals_as_printf_does;

    fn format_with_four_and_six_decimals(value: f64) -> (String, String) {
        (
            format_float_with_decimals_as_printf_does(value, 4),
            format_float_with_decimals_as_printf_does(value, 6),
        )
    }

    #[test]
    fn ordinary_values_print_as_printf_prints_them_with_four_and_six_decimals() {
        let expected_outputs = [
            (0.5, "0.5000", "0.500000"),
            (1.0, "1.0000", "1.000000"),
            (7.891011121, "7.8910", "7.891011"),
            (-4.56789123, "-4.5679", "-4.567891"),
            (42.00004999, "42.0000", "42.000050"),
            (-0.99999, "-1.0000", "-0.999990"),
            (123.456, "123.4560", "123.456000"),
            (0.1, "0.1000", "0.100000"),
            (1920.0, "1920.0000", "1920.000000"),
            (1080.5, "1080.5000", "1080.500000"),
            (0.35, "0.3500", "0.350000"),
            (2.675, "2.6750", "2.675000"),
            (1.005, "1.0050", "1.005000"),
            (0.1f32 as f64, "0.1000", "0.100000"),
            (0.35f32 as f64, "0.3500", "0.350000"),
        ];

        for (value, expected_four_decimals, expected_six_decimals) in expected_outputs {
            assert_eq!(
                format_with_four_and_six_decimals(value),
                (
                    expected_four_decimals.to_string(),
                    expected_six_decimals.to_string()
                ),
                "formatting {value:e}"
            );
        }
    }

    #[test]
    fn values_exactly_halfway_between_two_outputs_round_to_the_even_digit_as_printf_does() {
        let expected_outputs = [
            (0.03125, "0.0312", "0.031250"),
            (0.09375, "0.0938", "0.093750"),
            (0.15625, "0.1562", "0.156250"),
            (2.40625, "2.4062", "2.406250"),
            (-0.03125, "-0.0312", "-0.031250"),
            (-0.09375, "-0.0938", "-0.093750"),
            (0.0078125, "0.0078", "0.007812"),
            (0.0234375, "0.0234", "0.023438"),
            (-0.0078125, "-0.0078", "-0.007812"),
            (-0.0234375, "-0.0234", "-0.023438"),
        ];

        for (value, expected_four_decimals, expected_six_decimals) in expected_outputs {
            assert_eq!(
                format_with_four_and_six_decimals(value),
                (
                    expected_four_decimals.to_string(),
                    expected_six_decimals.to_string()
                ),
                "formatting {value:e}"
            );
        }
    }

    #[test]
    fn decimal_literals_that_look_halfway_round_by_their_exact_binary_value_as_printf_does() {
        let expected_outputs = [
            (0.00005, "0.0001", "0.000050"),
            (0.00015, "0.0001", "0.000150"),
            (1.00005, "1.0001", "1.000050"),
            (0.0000005, "0.0000", "0.000000"),
            (0.0000015, "0.0000", "0.000002"),
        ];

        for (value, expected_four_decimals, expected_six_decimals) in expected_outputs {
            assert_eq!(
                format_with_four_and_six_decimals(value),
                (
                    expected_four_decimals.to_string(),
                    expected_six_decimals.to_string()
                ),
                "formatting {value:e}"
            );
        }
    }

    #[test]
    fn very_large_magnitudes_print_every_integer_digit_as_printf_does() {
        let expected_outputs = [
            (
                1e20,
                "100000000000000000000.0000",
                "100000000000000000000.000000",
            ),
            (
                123456789012345678901234567890.0,
                "123456789012345677877719597056.0000",
                "123456789012345677877719597056.000000",
            ),
            (
                9007199254740993.0,
                "9007199254740992.0000",
                "9007199254740992.000000",
            ),
            (
                1e300,
                "1000000000000000052504760255204420248704468581108159154915854115511802457988908195786371375080447864043704443832883878176942523235360430575644792184786706982848387200926575803737830233794788090059368953234970799945081119038967640880074652742780142494579258788820056842838115669472196386865459400540160.0000",
                "1000000000000000052504760255204420248704468581108159154915854115511802457988908195786371375080447864043704443832883878176942523235360430575644792184786706982848387200926575803737830233794788090059368953234970799945081119038967640880074652742780142494579258788820056842838115669472196386865459400540160.000000",
            ),
            (
                f64::MAX,
                "179769313486231570814527423731704356798070567525844996598917476803157260780028538760589558632766878171540458953514382464234321326889464182768467546703537516986049910576551282076245490090389328944075868508455133942304583236903222948165808559332123348274797826204144723168738177180919299881250404026184124858368.0000",
                "179769313486231570814527423731704356798070567525844996598917476803157260780028538760589558632766878171540458953514382464234321326889464182768467546703537516986049910576551282076245490090389328944075868508455133942304583236903222948165808559332123348274797826204144723168738177180919299881250404026184124858368.000000",
            ),
        ];

        for (value, expected_four_decimals, expected_six_decimals) in expected_outputs {
            assert_eq!(
                format_with_four_and_six_decimals(value),
                (
                    expected_four_decimals.to_string(),
                    expected_six_decimals.to_string()
                ),
                "formatting {value:e}"
            );
        }
    }

    #[test]
    fn very_small_magnitudes_round_to_zero_and_keep_a_negative_sign_as_printf_does() {
        let expected_outputs = [
            (1e-5, "0.0000", "0.000010"),
            (1e-300, "0.0000", "0.000000"),
            (5e-324, "0.0000", "0.000000"),
            (4.9999e-5, "0.0000", "0.000050"),
            (5.0000001e-5, "0.0001", "0.000050"),
            (-1e-10, "-0.0000", "-0.000000"),
            (-4e-5, "-0.0000", "-0.000040"),
        ];

        for (value, expected_four_decimals, expected_six_decimals) in expected_outputs {
            assert_eq!(
                format_with_four_and_six_decimals(value),
                (
                    expected_four_decimals.to_string(),
                    expected_six_decimals.to_string()
                ),
                "formatting {value:e}"
            );
        }
    }

    #[test]
    fn positive_and_negative_zero_print_with_their_sign_as_printf_does() {
        assert_eq!(
            format_with_four_and_six_decimals(0.0),
            ("0.0000".to_string(), "0.000000".to_string())
        );
        assert_eq!(
            format_with_four_and_six_decimals(-0.0),
            ("-0.0000".to_string(), "-0.000000".to_string())
        );
    }

    #[test]
    fn not_a_number_prints_as_lowercase_nan_whatever_its_sign_as_printf_does() {
        assert_eq!(
            format_with_four_and_six_decimals(f64::NAN),
            ("nan".to_string(), "nan".to_string())
        );
        assert_eq!(
            format_with_four_and_six_decimals(-f64::NAN),
            ("nan".to_string(), "nan".to_string())
        );
    }

    #[test]
    fn infinities_print_as_inf_and_minus_inf_as_printf_does() {
        assert_eq!(
            format_with_four_and_six_decimals(f64::INFINITY),
            ("inf".to_string(), "inf".to_string())
        );
        assert_eq!(
            format_with_four_and_six_decimals(f64::NEG_INFINITY),
            ("-inf".to_string(), "-inf".to_string())
        );
    }
}
