use crate::space::manager::SpaceManager;
use crate::support::handles::SpaceId;

pub(crate) struct SpaceLabel {
    pub(crate) space_id: SpaceId,
    pub(crate) label: String,
}

pub(crate) fn label_of_space(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
) -> Option<&mut SpaceLabel> {
    space_manager
        .labels
        .iter_mut()
        .find(|space_label| space_label.space_id == space_id)
}

pub(crate) fn space_label_with_name<'space_manager>(
    space_manager: &'space_manager mut SpaceManager,
    label: &[u8],
) -> Option<&'space_manager mut SpaceLabel> {
    let label = String::from_utf8_lossy(label);

    space_manager
        .labels
        .iter_mut()
        .find(|space_label| space_label.label == label)
}

pub(crate) fn remove_label_of_space(space_manager: &mut SpaceManager, space_id: SpaceId) -> bool {
    for index in 0..space_manager.labels.len() {
        let space_label = &space_manager.labels[index];
        if space_label.space_id == space_id {
            space_manager.labels.swap_remove(index);
            return true;
        }
    }

    false
}

pub(crate) fn set_label_of_space_removing_it_from_any_other_space(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    label: String,
) {
    remove_label_of_space(space_manager, space_id);

    for index in 0..space_manager.labels.len() {
        let space_label = &space_manager.labels[index];
        if space_label.label == label {
            space_manager.labels.swap_remove(index);
            break;
        }
    }

    space_manager.labels.push(SpaceLabel { space_id, label });
}
