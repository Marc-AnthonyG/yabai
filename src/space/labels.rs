use crate::handles::SpaceId;
use crate::space::manager::SpaceManager;

pub(crate) struct SpaceLabel {
    pub(crate) space_id: SpaceId,
    pub(crate) label: String,
}

pub(crate) fn space_manager_get_label_for_space(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
) -> Option<&mut SpaceLabel> {
    space_manager
        .labels
        .iter_mut()
        .find(|space_label| space_label.space_id == space_id)
}

pub(crate) fn space_manager_get_space_for_label<'space_manager>(
    space_manager: &'space_manager mut SpaceManager,
    label: &[u8],
) -> Option<&'space_manager mut SpaceLabel> {
    space_manager
        .labels
        .iter_mut()
        .find(|space_label| space_label.label.as_bytes() == label)
}

pub(crate) fn space_manager_remove_label_for_space(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
) -> bool {
    for index in 0..space_manager.labels.len() {
        let space_label = &space_manager.labels[index];
        if space_label.space_id == space_id {
            space_manager.labels.swap_remove(index);
            return true;
        }
    }

    false
}

pub(crate) fn space_manager_set_label_for_space(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    label: String,
) {
    space_manager_remove_label_for_space(space_manager, space_id);

    for index in 0..space_manager.labels.len() {
        let space_label = &space_manager.labels[index];
        if space_label.label == label {
            space_manager.labels.swap_remove(index);
            break;
        }
    }

    space_manager.labels.push(SpaceLabel { space_id, label });
}
