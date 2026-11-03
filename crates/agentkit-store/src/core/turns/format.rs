use chrono::{Datelike, Timelike};
/// Ported from mkagent v0.2.6 packages/ui/src/components/chat/turn-utils.ts (formatDuration).
pub fn format_duration(ms: f64) -> String {
    if !ms.is_finite() || ms < 0.0 {
        return "--".into();
    }
    let seconds = ms / 1000.0;
    if seconds < 60.0 {
        return format!("{}s", fixed_one(seconds));
    }
    let minutes = (seconds / 60.0).floor();
    if minutes >= 2.0 {
        format!("{}m+", integer_string(minutes))
    } else {
        format!("{minutes}m {}s", (seconds % 60.0).round())
    }
}
/// Ported from mkagent v0.2.6 packages/ui/src/components/chat/turn-utils.ts (formatTokens).
pub fn format_tokens(count: f64) -> String {
    if !count.is_finite() || count < 0.0 {
        return "0".into();
    }
    // JavaScript Math.floor(-0).toString() displays "0".
    let count = count.floor().abs();
    if count < 1000.0 {
        format!("{count:.0}")
    } else if count < 10000.0 {
        format!("{}k", fixed_one(count / 1000.0))
    } else {
        format!("{}k", integer_string((count / 1000.0).round()))
    }
}
/// Ported from mkagent v0.2.6 packages/ui/src/components/chat/message-timestamp.ts.
/// Caller supplies the local UTC offset at this instant (including DST); no global timezone IO.
pub fn format_message_timestamp(timestamp_ms: i64, utc_offset_seconds: i32) -> Option<String> {
    chrono::FixedOffset::east_opt(utc_offset_seconds)?;
    let date = chrono::DateTime::from_timestamp_millis(timestamp_ms)?
        .naive_utc()
        .checked_add_signed(chrono::TimeDelta::seconds(i64::from(utc_offset_seconds)))?;
    Some(format!(
        "{}-{:02}-{:02} {:02}:{:02}:{:02}",
        date.year(),
        date.month(),
        date.day(),
        date.hour(),
        date.minute(),
        date.second()
    ))
}

// Number.toFixed(1) rounds the exact positive binary value; Rust formatting uses
// ties-to-even. Inputs here are finite and below 60. Preserve the JS tie rule.
fn fixed_one(value: f64) -> String {
    let bits = value.to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as i32;
    let significand = (bits & ((1_u64 << 52) - 1)) | if exponent == 0 { 0 } else { 1_u64 << 52 };
    let shift = if exponent == 0 { 1074 } else { 1075 - exponent };
    let numerator = u128::from(significand) * 10;
    let rounded = if shift >= 128 {
        0
    } else {
        (numerator + (1_u128 << (shift - 1))) >> shift
    };
    format!("{}.{}", rounded / 10, rounded % 10)
}
// JS Number.toString switches to exponent notation at 1e21; these values are
// nonnegative integral counts. Rust shortest formatting supplies the same digits.
fn integer_string(value: f64) -> String {
    if value >= 1e21 {
        let s = format!("{value:e}");
        s.replace('e', "e+")
    } else {
        value.to_string()
    }
}
