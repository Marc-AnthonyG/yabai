use crate::display::manager::DisplayManager;
use crate::handles::DisplayId;
use crate::support::strings::string_equals;

pub(crate) struct DisplayLabel {
    pub(crate) display_id: DisplayId,
    pub(crate) label: String,
}

pub(crate) fn display_manager_get_label_for_display(
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

pub(crate) fn display_manager_get_display_for_label<'display_manager>(
    display_manager: &'display_manager mut DisplayManager,
    label: &[u8],
) -> Option<&'display_manager mut DisplayLabel> {
    let label = String::from_utf8_lossy(label);

    for display_label in display_manager.labels.iter_mut() {
        if string_equals(Some(&label), Some(&display_label.label)) {
            return Some(display_label);
        }
    }

    None
}

pub(crate) fn display_manager_remove_label_for_display(
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

pub(crate) fn display_manager_set_label_for_display(
    display_manager: &mut DisplayManager,
    display_id: DisplayId,
    label: String,
) {
    display_manager_remove_label_for_display(display_manager, display_id);

    for index in 0..display_manager.labels.len() {
        let display_label = &display_manager.labels[index];
        if display_label.label == label {
            display_manager.labels.swap_remove(index);
            break;
        }
    }

    display_manager.labels.push(DisplayLabel { display_id, label });
}
