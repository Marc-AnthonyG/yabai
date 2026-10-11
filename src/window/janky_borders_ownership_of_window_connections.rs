use std::collections::HashMap;

use crate::window::janky_borders::is_window_connection_owned_by_janky_borders;

const MOST_WINDOW_CONNECTIONS_REMEMBERED_BEFORE_FORGETTING_THEM_ALL: usize = 64;

#[derive(Default)]
pub(crate) struct JankyBordersOwnershipOfWindowConnections {
    is_owned_by_janky_borders_by_window_connection: HashMap<i32, bool>,
}

impl JankyBordersOwnershipOfWindowConnections {
    pub(crate) fn is_owned_by_janky_borders(&mut self, window_connection_id: i32) -> bool {
        self.remembered_or_asked_once(
            window_connection_id,
            is_window_connection_owned_by_janky_borders,
        )
    }

    fn remembered_or_asked_once(
        &mut self,
        window_connection_id: i32,
        ask_the_window_server_and_the_kernel: impl FnOnce(i32) -> bool,
    ) -> bool {
        if let Some(is_owned_by_janky_borders) = self
            .is_owned_by_janky_borders_by_window_connection
            .get(&window_connection_id)
        {
            return *is_owned_by_janky_borders;
        }
        if self.is_owned_by_janky_borders_by_window_connection.len()
            >= MOST_WINDOW_CONNECTIONS_REMEMBERED_BEFORE_FORGETTING_THEM_ALL
        {
            self.is_owned_by_janky_borders_by_window_connection.clear();
        }
        let is_owned_by_janky_borders = ask_the_window_server_and_the_kernel(window_connection_id);
        self.is_owned_by_janky_borders_by_window_connection
            .insert(window_connection_id, is_owned_by_janky_borders);
        is_owned_by_janky_borders
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::{
        JankyBordersOwnershipOfWindowConnections,
        MOST_WINDOW_CONNECTIONS_REMEMBERED_BEFORE_FORGETTING_THEM_ALL,
    };

    const JANKY_BORDERS_CONNECTION: i32 = 0x1_2345;
    const TERMINAL_CONNECTION: i32 = 0x2_3456;

    #[test]
    fn each_window_connection_is_asked_about_once_and_its_answer_remembered() {
        let mut ownership = JankyBordersOwnershipOfWindowConnections::default();
        let questions_asked = Cell::new(0);
        let ask = |window_connection_id| {
            questions_asked.set(questions_asked.get() + 1);
            window_connection_id == JANKY_BORDERS_CONNECTION
        };

        for _ in 0..3 {
            assert!(ownership.remembered_or_asked_once(JANKY_BORDERS_CONNECTION, ask));
            assert!(!ownership.remembered_or_asked_once(TERMINAL_CONNECTION, ask));
        }

        assert_eq!(questions_asked.get(), 2);
    }

    #[test]
    fn a_full_memory_of_window_connections_is_forgotten_before_the_next_one_is_asked_about() {
        let mut ownership = JankyBordersOwnershipOfWindowConnections::default();
        let questions_asked = Cell::new(0);
        let ask = |_| {
            questions_asked.set(questions_asked.get() + 1);
            false
        };
        let connection_count = MOST_WINDOW_CONNECTIONS_REMEMBERED_BEFORE_FORGETTING_THEM_ALL as i32;

        for window_connection_id in 0..=connection_count {
            ownership.remembered_or_asked_once(window_connection_id, ask);
        }
        ownership.remembered_or_asked_once(connection_count, ask);
        ownership.remembered_or_asked_once(0, ask);

        assert_eq!(questions_asked.get(), connection_count as usize + 2);
    }
}
