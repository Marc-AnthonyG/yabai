use crate::display::manager::DisplayManager;
use crate::support::handles::DisplayId;
use crate::support::strings::are_both_strings_present_and_equal;

pub(crate) struct DisplayLabel {
    pub(crate) display_id: DisplayId,
    pub(crate) label: String,
}

pub(crate) fn label_of_display(
    display_manager: &mut DisplayManager,
    display_id: DisplayId,
) -> Option<&mut DisplayLabel> {
    for display_label in display_manager.labels.iter_mut() {
        if display_label.display_id == display_id {
            return Some(display_label);
        }
    }

    None
}

pub(crate) fn display_label_with_name<'display_manager>(
    display_manager: &'display_manager mut DisplayManager,
    label: &[u8],
) -> Option<&'display_manager mut DisplayLabel> {
    let label = String::from_utf8_lossy(label);

    for display_label in display_manager.labels.iter_mut() {
        if are_both_strings_present_and_equal(Some(&label), Some(&display_label.label)) {
            return Some(display_label);
        }
    }

    None
}

pub(crate) fn remove_label_of_display(
    display_manager: &mut DisplayManager,
    display_id: DisplayId,
) -> bool {
    for index in 0..display_manager.labels.len() {
        let display_label = &display_manager.labels[index];
        if display_label.display_id == display_id {
            display_manager.labels.swap_remove(index);
            return true;
        }
    }

    false
}

pub(crate) fn set_label_of_display_removing_it_from_any_other_display(
    display_manager: &mut DisplayManager,
    display_id: DisplayId,
    label: String,
) {
    remove_label_of_display(display_manager, display_id);

    for index in 0..display_manager.labels.len() {
        let display_label = &display_manager.labels[index];
        if display_label.label == label {
            display_manager.labels.swap_remove(index);
            break;
        }
    }

    display_manager
        .labels
        .push(DisplayLabel { display_id, label });
}
