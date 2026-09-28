pub(crate) fn message_socket_path_of_user(user: &str) -> String {
    format!("/tmp/yabai_{user}.socket")
}
