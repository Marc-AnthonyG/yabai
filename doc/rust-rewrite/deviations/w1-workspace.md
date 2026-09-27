# Deviations — `w1-workspace` (W1-workspace, `src/workspace.h` + `src/workspace.m` → `src/workspace.rs`)

`src/workspace.m:7-12`, `src/yabai.c:47` | `[workspace_context alloc]` was stored into the `void **context` out-parameter, which `workspace_event_handler_begin`'s only caller points at the `g_workspace_context` global | the out-parameter is gone and the function fills the static itself (`state-access/workspace-sa-main.md` §8.2, `patterns/state-and-ownership.md` §2.1); the `bool` return that `src/yabai.c:295` turns into `error(...)` is kept

`src/yabai.c:47` | `g_workspace_context` was a bare `void *` global | `pub(crate) static WORKSPACE_CONTEXT: OnceLock<SendRetained<WorkspaceContext>>`, not the `OnceLock<Retained<WorkspaceContext>>` of `GLOSSARY.md` §8.1: `define_class!` propagates the ivars' auto traits through a `PhantomData<(Ivars, …)>` (`objc2-0.6.4/src/macros/define_class.rs:538-543`), and `WorkspaceContextIvars` holds an `mpsc::Sender<Event>`, which is `Send` but not `Sync`, so `Retained<WorkspaceContext>` is not `Sync` and the static does not compile; the on-disk `SendRetained` of `src/misc/notify.rs:17-19` is used instead (`DECISIONS.md` 42)

`src/workspace.m:11` | `[ws_context init]` was called for its side effects and its result discarded, the `alloc` pointer being stored instead | the value `init` returns is what is stored (`patterns/ffi-objc-and-os.md` §20.1)

`src/workspace.h:21-24`, `src/workspace.m:152` | `@interface workspace_context : NSObject { }` declared no instance variables and the class reached the event queue through the `g_event_loop` global | the class carries `WorkspaceContextIvars { event_sender: Sender<Event> }`, since `DECISIONS.md` 12 moves the event loop into `EventLoopOwnedState` and `patterns/ffi-objc-and-os.md` §20.2 puts the sender in the ivars

`src/workspace.h:13` | the X-macro declared six `static bool _workspace_is_macos_version_<name>` | six `static _workspace_is_macos_version_<name>: AtomicBool`, keeping the C's lowercase names per `GLOSSARY.md` §8.2 and `state-access/workspace-sa-main.md` §7, against `GLOSSARY.md`'s own SCREAMING_SNAKE rule for file-scope statics and against the `WORKSPACE_IS_MACOS_VERSION_*` of `patterns/ffi-objc-and-os.md` §20.4; the module carries `#![allow(non_upper_case_globals)]` for them

`src/workspace.m:4-6` | the `SUPPORTED_MACOS_VERSION_LIST` expansion assigned all six flags inline in `workspace_event_handler_begin` | `macro_rules! supported_macos_version_list` emits only the six statics and the six `workspace_is_macos_<name>()` accessors; the `set_supported_macos_version_flags` function of `patterns/ffi-objc-and-os.md` §20.4 is not emitted, because `state-access/workspace-sa-main.md` §3 rules that the flag write has no function of its own and is written inline at this position

`GLOSSARY.md` §2.5 | — | the `MacosVersion` type that `GLOSSARY.md` §2.5 lists for `crate::workspace` is not declared: `GLOSSARY.md` §8.2 and `state-access/workspace-sa-main.md` §7 both fix the six version flags as six `AtomicBool` statics behind six accessors, leaving the type with nothing to hold

`src/workspace.m:293`, `:299` | `didHideApplication:` and `didUnhideApplication:` punned the `pid_t` into the event payload with `(void *)(intptr_t) pid` | `Event::ApplicationHidden(ProcessId)` and `Event::ApplicationVisible(ProcessId)` carry the value (`DECISIONS.md` 19, `THREADS.md` §3.2)
