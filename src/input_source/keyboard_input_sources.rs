use crate::ffi::carbon_events::noErr;
use crate::ffi::core_foundation::{
    CFArray, CFBoolean, CFDictionary, CFRetained, CFString, CFType, cfarray_borrow_value_at_index,
    cfarray_count, take_create_rule_result,
};
use crate::ffi::text_input_sources::{
    TISCopyCurrentKeyboardInputSource, TISCreateInputSourceList, TISGetInputSourceProperty,
    TISSelectInputSource, kTISCategoryKeyboardInputSource, kTISPropertyInputSourceCategory,
    kTISPropertyInputSourceID, kTISPropertyInputSourceIsEnabled,
    kTISPropertyInputSourceIsSelectCapable, kTISPropertyInputSourceType, kTISTypeKeyboardInputMode,
    kTISTypeKeyboardLayout,
};

pub(crate) fn select_the_keyboard_input_source_after_the_current_one() -> Result<(), String> {
    let enabled_keyboard_input_sources = input_sources_matching(
        unsafe { kTISPropertyInputSourceCategory },
        unsafe { kTISCategoryKeyboardInputSource },
        false,
    );
    let input_sources_to_cycle_through: Vec<&CFType> =
        input_sources_of_list(&enabled_keyboard_input_sources)
            .filter(|input_source| {
                is_an_enabled_layout_or_input_mode_that_can_be_selected(input_source)
            })
            .collect();
    if input_sources_to_cycle_through.is_empty() {
        return Err(String::from(
            "no keyboard layout or input mode is enabled and can be selected.",
        ));
    }

    let current_input_source_id = current_keyboard_input_source_id();
    let position_of_the_current_input_source = input_sources_to_cycle_through
        .iter()
        .position(|input_source| Some(input_source_id(input_source)) == current_input_source_id);
    let position_of_the_next_input_source = position_of_the_current_input_source
        .map_or(0, |position| {
            (position + 1) % input_sources_to_cycle_through.len()
        });
    select_input_source(input_sources_to_cycle_through[position_of_the_next_input_source])
}

pub(crate) fn check_that_the_input_source_can_be_selected(
    input_source_id: &str,
) -> Result<(), String> {
    with_the_input_source_that_can_be_selected(input_source_id, |_| Ok(()))
}

pub(crate) fn select_the_input_source_with_id(input_source_id: &str) -> Result<(), String> {
    with_the_input_source_that_can_be_selected(input_source_id, select_input_source)
}

fn with_the_input_source_that_can_be_selected(
    input_source_id: &str,
    use_the_input_source: impl FnOnce(&CFType) -> Result<(), String>,
) -> Result<(), String> {
    let installed_input_sources_with_the_id = input_sources_matching(
        unsafe { kTISPropertyInputSourceID },
        &CFString::from_str(input_source_id),
        true,
    );
    let input_source = input_sources_of_list(&installed_input_sources_with_the_id)
        .next()
        .ok_or_else(|| format!("input source '{input_source_id}' is not installed."))?;
    let is_enabled =
        boolean_property_of_input_source(input_source, unsafe { kTISPropertyInputSourceIsEnabled });
    if !is_enabled {
        return Err(format!(
            "input source '{input_source_id}' is not enabled in System Settings."
        ));
    }
    let can_be_selected = boolean_property_of_input_source(input_source, unsafe {
        kTISPropertyInputSourceIsSelectCapable
    });
    if !can_be_selected {
        return Err(format!(
            "input source '{input_source_id}' cannot be selected."
        ));
    }
    use_the_input_source(input_source)
}

fn is_an_enabled_layout_or_input_mode_that_can_be_selected(input_source: &CFType) -> bool {
    let input_source_type =
        string_property_of_input_source(input_source, unsafe { kTISPropertyInputSourceType });
    let is_a_layout_or_an_input_mode = input_source_type.is_some_and(|input_source_type| {
        input_source_type == unsafe { kTISTypeKeyboardLayout }
            || input_source_type == unsafe { kTISTypeKeyboardInputMode }
    });
    is_a_layout_or_an_input_mode
        && boolean_property_of_input_source(input_source, unsafe {
            kTISPropertyInputSourceIsEnabled
        })
        && boolean_property_of_input_source(input_source, unsafe {
            kTISPropertyInputSourceIsSelectCapable
        })
}

fn select_input_source(input_source: &CFType) -> Result<(), String> {
    let status = unsafe { TISSelectInputSource(input_source) };
    if status != noErr {
        return Err(format!(
            "could not select input source '{}' (status {status}).",
            input_source_id(input_source)
        ));
    }
    Ok(())
}

fn current_keyboard_input_source_id() -> Option<String> {
    let current_input_source =
        unsafe { take_create_rule_result(TISCopyCurrentKeyboardInputSource()) }?;
    Some(input_source_id(&current_input_source))
}

fn input_source_id(input_source: &CFType) -> String {
    string_property_of_input_source(input_source, unsafe { kTISPropertyInputSourceID })
        .map(ToString::to_string)
        .unwrap_or_default()
}

fn input_sources_matching(
    property_key: &CFString,
    property_value: &CFString,
    include_the_installed_ones_that_are_not_enabled: bool,
) -> Option<CFRetained<CFArray>> {
    let filter = CFDictionary::from_slices(&[property_key], &[property_value]);
    unsafe {
        take_create_rule_result(TISCreateInputSourceList(
            filter.as_opaque(),
            u8::from(include_the_installed_ones_that_are_not_enabled),
        ))
    }
}

fn input_sources_of_list(
    input_sources: &Option<CFRetained<CFArray>>,
) -> impl Iterator<Item = &CFType> {
    let input_sources = input_sources.as_deref();
    let input_source_count = input_sources.map_or(0, cfarray_count);
    (0..input_source_count).filter_map(move |index| {
        input_sources.and_then(|input_sources| unsafe {
            cfarray_borrow_value_at_index::<CFType>(input_sources, index)
        })
    })
}

fn string_property_of_input_source<'input_source>(
    input_source: &'input_source CFType,
    property_key: &CFString,
) -> Option<&'input_source CFString> {
    unsafe {
        TISGetInputSourceProperty(input_source, property_key)
            .cast::<CFString>()
            .as_ref()
    }
}

fn boolean_property_of_input_source(input_source: &CFType, property_key: &CFString) -> bool {
    unsafe {
        TISGetInputSourceProperty(input_source, property_key)
            .cast::<CFBoolean>()
            .as_ref()
    }
    .is_some_and(CFBoolean::as_bool)
}
