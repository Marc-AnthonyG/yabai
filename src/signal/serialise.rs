use crate::signal::definition::{
    SIGNAL_TYPE_BY_DISCRIMINANT, SIGNAL_TYPE_COUNT, SIGNAL_TYPE_STR, Signal, SignalType,
};
use crate::support::json::{json_optional_bool, ts_string_escape};
use crate::support::response::Response;

pub(crate) fn event_signal_serialize(
    response: &mut Response,
    signal: &Signal,
    signal_type: SignalType,
    index: i32,
) {
    let app = signal.app.as_deref();
    let title = signal.title.as_deref();
    let command = signal.command.as_deref();

    let escaped_app = app.and_then(ts_string_escape);
    let escaped_title = title.and_then(ts_string_escape);
    let escaped_command = command.and_then(ts_string_escape);

    response.write(format_args!(
        "{{\n\t\"index\":{},\n\t\"label\":\"{}\",\n\t\"app\":\"{}\",\n\t\"title\":\"{}\",\n\t\"active\":{},\n\t\"event\":\"{}\",\n\t\"action\":\"{}\"\n}}",
        index,
        signal.label.as_deref().unwrap_or(""),
        escaped_app.as_deref().or(app).unwrap_or(""),
        escaped_title.as_deref().or(title).unwrap_or(""),
        json_optional_bool(signal.active as i32),
        SIGNAL_TYPE_STR[signal_type as usize],
        escaped_command.as_deref().or(command).unwrap_or("")
    ));
}

pub(crate) fn event_signal_list(
    response: &mut Response,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
) {
    response.write(format_args!("["));
    let mut signal_index: i32 = 0;
    let mut event_did_output = false;
    for index in SignalType::ApplicationLaunched as usize..SIGNAL_TYPE_COUNT {
        let count = signal_event[index].len() as i32;

        if count > 0 && event_did_output {
            response.write(format_args!(","));
        }

        for inner_index in 0..count {
            event_signal_serialize(
                response,
                &signal_event[index][inner_index as usize],
                SIGNAL_TYPE_BY_DISCRIMINANT[index],
                signal_index,
            );
            if inner_index < signal_event[index].len() as i32 - 1 {
                response.write(format_args!(","));
            }
            signal_index += 1;
        }

        if !event_did_output {
            event_did_output = count > 0;
        }
    }
    response.write(format_args!("]\n"));
}
