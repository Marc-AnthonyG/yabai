# Deviations — `w2-process_manager-part1` (W2-process_manager, bodies of `src/process_manager.rs`)

`src/process_manager.c:119-120` | `query` and `iterator` are passed to `SLSWindowIteratorAdvance` and to `CFRelease` without a NULL check, so a failed `SLSWindowQueryWindows` or `SLSWindowQueryResultCopyWindows` dereferences NULL | each result is checked and `process_manager_active_space_for_psn` returns the space id it has, releasing exactly what the C `out:`/`err:` ladder would have released (`DECISIONS.md` 4, `files/window-application-process.md` §6.6(g))

`src/process_manager.c:87-88` | `display_manager_active_display_list` returned a `ts` arena pointer and the caller returned early when it was NULL | the Rust returns a `Vec<DisplayId>`, which cannot be NULL, so the early return has no translation (`DECISIONS.md` 17)

`src/process_manager.c:99-106` | the per-display space lists were consecutive `ts` arena allocations, tracked as the first pointer plus a running total | one `Vec<u64>` extended once per display, which is the `DECISIONS.md` 17 treatment of an arena site that concatenates consecutive allocations; the `NOTE(asmvik)` comment is carried over at the accumulation

`src/process_manager.c:70-80` | `process_is_being_debugged` sysctls a `struct kinfo_proc` with only `kp_proc.p_flag` zeroed | not translated into `src/process_manager.rs`: `src/ffi/libsystem.rs` already defines `process_is_being_debugged(process_id: pid_t) -> bool` and the three call sites call it there (`DECISIONS.md` 42; the zeroing itself is recorded by `w0-ffi-carbon-mach-libsystem`)

`src/process_manager.c:155`, `:246` | `ProcessSerialNumber psn` and `ProcessSerialNumber front_psn` are declared uninitialised and filled by `GetEventParameter` / `_SLPSGetFrontProcess` | both are zeroed at their declaration, Rust having no uninitialised binding; neither is on a path where the C read uninitialised memory (`DECISIONS.md` 4)

`src/process_manager.c:232-239`, `:251` | `target`, `handler`, `type[3]` and `ref` were written into `struct process_manager` before `InstallEventHandler` read `pm->type` through the manager | the four are locals and are published together into `CARBON_PROCESS_EVENT_INSTALLATION` once the install returns, because the static holds the whole struct and `handler_ref` is only known then; the event type list reaches Carbon as a pointer to the local array, which `InstallEventHandler` copies (`GLOSSARY.md` §3.5)

`src/process_manager.c:251` | `InstallEventHandler` was given `pm` (`&g_process_manager`) as the handler refcon | the refcon is NULL: `process_handler` reaches the process table through the `PROCESS_TABLE` static and never dereferences `context`, which survives only as the unused ABI slot (`state-access/window-application-process.md` §3)

`src/process_manager.c:160-202` | the `switch` on `GetEventKind(event)` compares against the Carbon `kEventApp*` constants | the `match` uses the same constants as patterns, so `process_handler` carries `#[allow(non_upper_case_globals)]`, which `rustc` would otherwise lint under `nonstandard_style` (`DECISIONS.md` 37, as `w0-ffi-carbon-mach-libsystem` did for the declarations)
