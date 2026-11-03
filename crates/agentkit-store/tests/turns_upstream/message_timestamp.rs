//! Named counterparts of the original upstream test cases.
#[test]
fn case_0_formats_a_timestamp_in_local_time_with_zero_padded_fields() {
    super::suite(
        "message-timestamp.test.ts",
        "formats a timestamp in local time with zero-padded fields",
    );
}
