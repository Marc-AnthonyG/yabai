# Signature changes — `w2b-objc` (bodies of `src/workspace.rs` and `src/sa.rs`)

No frozen signature was changed, in these two files or in any other module. One item was
**added**, private to its module:

`src/sa.m:80-93`, `:126-135`, `:158`, `:196` | — | `fn truncate_as_snprintf_would_into_a_maxlen_buffer(text: String) -> String` in `src/sa.rs` | the seventeen `snprintf(buffer, MAXLEN, …)` sites (the eleven `osax_*` paths, the four `chmod` / `codesign` commands, the `rm -rf` command and `g_sa_socket_file`) truncate at 511 bytes, which `state-access/workspace-sa-main.md` §5.1 and `patterns/ffi-objc-and-os.md` §26.3 require the Rust to reproduce at the same points; no helper for it exists under `src/misc/`

One request for a module this unit does not own:

`src/workspace.m:38` | `src/ffi/foundation.rs` re-exports `NSObjectNSKeyValueObserverRegistration` and `NSObjectNSKeyValueCoding` but not `NSObjectNSKeyValueObservingCustomization` | add `NSObjectNSKeyValueObservingCustomization` to the `objc2_foundation` re-export list | `-observationInfo` lives in that category trait; until it is re-exported, `workspace_application_destroy_running_ns_application` sends it with `msg_send![&*application, observationInfo]`, which is the same message and can stay as it is
