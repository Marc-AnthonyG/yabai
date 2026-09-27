# Deviations — `w1-process_manager` (W1-process_manager, `src/process_manager.h` + `src/process_manager.c` → `src/process_manager.rs`)

`src/process_manager.h:13` | `int policy` was a plain `int`, written and read from the event-loop thread and from the main-thread KVO callback without synchronisation (`src/workspace.m:107`, `:110`, `:230`), and `process_create` (`src/process_manager.c:31-67`) left it uninitialised | `Process::policy` is an `AtomicI32` carrying the same values with relaxed orderings, and is initialised explicitly in `process_create` (`DECISIONS.md` 4, 22, 47)

`src/process_manager.c:44` | a NULL return from `cfstring_copy` was not checked, so `string_equals(process_name, ...)` at `:54` dereferenced NULL | `process_create` returns `None` when `cfstring_copy` returns `None`, so no `Process` is built and the blacklist comparison never runs (`DECISIONS.md` 4)

`doc/rust-rewrite/state-access/window-application-process.md:§3` | the visibility column marks `hash_psn`, `process_pid_for_psn`, `process_create`, `process_is_being_debugged`, `process_handler` and `process_manager_add_running_processes` private | all ten functions are `pub(crate)` (`DECISIONS.md` 48); phase 3 tightens visibility

`doc/rust-rewrite/GLOSSARY.md:§3.5` | the process table is given as `static PROCESS_TABLE: Mutex<Table<ProcessSerialNumber, Arc<Process>>>`, which has no constant initialiser because `Table::new` allocates | `static PROCESS_TABLE: OnceLock<Mutex<Table<ProcessSerialNumber, Arc<Process>>>>`, the shape `THREADS.md` §6.2 gives the same static under its superseded name, initialised where the C calls `table_init` (`src/process_manager.c:240`)

`doc/rust-rewrite/TRANSLATION_PLAN.md:§3.3 W1-process_manager` | the unit is sized at 4 fn, the four declarations of `src/process_manager.h:30-33` | `src/process_manager.rs` declares ten function signatures, one per row of `state-access/window-application-process.md` §3 for both C files; `compare_psn` contributes none, it is the derived `PartialEq` on `ProcessSerialNumber` in `src/ffi/carbon_process.rs`
