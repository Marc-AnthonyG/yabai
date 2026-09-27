# GLOSSARY — the binding spelling table

Every name in the Rust crate is fixed here. `DECISIONS.md` 37 requires one glossary; this is it.
A name that appears in this file is spelled that way and no other way, in every module, by every
translator. If a spelling is missing here, stop and add it here before writing the code.

Precedence, highest first: `DECISIONS.md` → this file → `patterns/*.md` → `THREADS.md` →
`files/*.md` and `sweeps/*.md` (phase-1 inventories, frequently superseded). Section 12 lists
every place where a `patterns/` or `THREADS.md` spelling disagrees with this file, and what won.

Rules this file applies, so they are not repeated per row:

* Functions keep their C name, `snake_case`, unchanged (`DECISIONS.md` 37).
* `struct foo_bar` → `FooBar`; `enum foo_bar` → `FooBar` (non-flag) or a newtype (OR-ed).
* Enum constants drop the C prefix and CamelCase the rest.
* Fields, parameters, locals and loop bindings are **never** abbreviated.
* `#define NAME "text"` keeps `NAME` exactly.
* File-scope `static` data becomes `SCREAMING_SNAKE`.

---

## 1. Handle newtypes — `src/handles.rs`, imported everywhere

`patterns/state-and-ownership.md` §3.1. The inner field is `pub`; every FFI call passes `.0`
explicitly. No `Deref`, no `From`, no implicit conversion in either direction.

| Rust type | wraps | C spellings it replaces | zero means |
| --- | --- | --- | --- |
| `WindowId` | `u32` | `uint32_t wid`, `window_id`, `a_wid`, `b_wid`, `filter_wid`, `rel_wid`, `struct window *` when stored | `WindowId(0)` = none |
| `ProcessId` | `i32` (`pid_t`) | `pid_t pid`, `front_pid`, `last_front_pid`, `struct application *` / `struct process *` when stored | `ProcessId(0)` = none |
| `SpaceId` | `u64` | `uint64_t sid`, `a_sid`, `b_sid`, `src_sid`, `dst_sid`, `struct view *` when stored | `SpaceId(0)` = none |
| `DisplayId` | `u32` | `uint32_t did`, `a_did`, `b_did`, `acting_did` | `DisplayId(0)` = none |
| `NodeId` | `u32` | `struct window_node *` inside one `View` | index into `View::nodes`; `0` is the root |
| `ProcessSerialNumber` | `#[repr(C)] { high_long_of_psn: u32, low_long_of_psn: u32 }` | `ProcessSerialNumber psn` | — |

| constant | value | note |
| --- | --- | --- |
| `ROOT_NODE_ID` | `NodeId(0)` | always valid for a live `View` (`DECISIONS.md` 15) |

A node handle that crosses a `View` boundary is the pair `(SpaceId, NodeId)`, never a bare
`NodeId`.

---

## 2. Every C type and its Rust type name

### 2.1 Structs

| C | Rust | module | note |
| --- | --- | --- | --- |
| `struct application` (`application.h:68`) | `Application` | `crate::application` | |
| `struct area` (`view.h:42`) | `Area` | `crate::view` | `f32` fields stay `f32` (`DECISIONS.md` 30) |
| `struct balance_node` (`view.h:88`) | `BalanceNode` | `crate::view` | |
| `struct bucket` (`misc/hashtable.h:10`) | **gone** | — | `Table` stores `Vec<(K, V)>` per bucket (`DECISIONS.md` 16) |
| `struct buf_hdr` (`misc/sbuffer.h:4`) | **gone** | — | `Vec` (`DECISIONS.md` 17) |
| `struct display_label` (`display_manager.h:36`) | `DisplayLabel` | `crate::display_manager` | |
| `struct display_manager` (`display_manager.h:42`) | `DisplayManager` | `crate::display_manager` | |
| `struct event` (`event_loop.h:55`) | **gone** | — | replaced by `enum Event` (`DECISIONS.md` 19) |
| `struct event_loop` (`event_loop.h:63`) | **gone** | — | `mpsc` channel; `EVENT_SENDER` static |
| `struct event_signal` (`event_signal.h:94`) | `PendingSignal` | `crate::event_signal` | the *queued* record; owns its strings (`DECISIONS.md` 17) |
| `struct feedback_window` (`view.h:145`) | `FeedbackWindow` | `crate::view` | |
| `struct memory_pool` (`misc/memory_pool.h:4`) | **gone** | — | owned queue (`DECISIONS.md` 17) |
| `struct mouse_state` (`mouse_handler.h:62`) | **split**: `MouseTapState` + `MouseDragState` | `crate::mouse_handler`, `crate::state` | `DECISIONS.md` 23 |
| `struct mouse_window_info` (`mouse_handler.h:53`) | `MouseWindowInfo` | `crate::mouse_handler` | |
| `struct process` (`process_manager.h:7`) | `Process`, always held as `Arc<Process>` | `crate::process_manager` | `DECISIONS.md` 22 |
| `struct process_manager` (`process_manager.h:17`) | **split**: `ProcessManager` + `PROCESS_TABLE` + `CarbonProcessEventInstallation` | `crate::process_manager` | §3.5 |
| `struct profile_anchor` (`misc/timer.h:8`) | **not translated** | — | dead code (`DECISIONS.md` 5) |
| `struct properties` (`message.c:605`) | `Properties` | `crate::message` | |
| `struct rgba_color` (`misc/helpers.h:162`) | `RgbaColor` | `crate::misc::helpers` | |
| `struct rule` (`rule.h:44`) | `Rule` | `crate::rule` | |
| `struct rule_effects` (`rule.h:29`) | `RuleEffects` | `crate::rule` | |
| `struct scratchpad` (`window_manager.h:68`) | `Scratchpad` | `crate::window_manager` | |
| `struct selector` (`message.c:652`) | `Selector<Target>` + `SelectorOutcome<Target>` | `crate::message` | |
| `struct signal` (`event_signal.h:104`) | `Signal` | `crate::event_signal` | the *subscription* record |
| `struct space_label` (`space_manager.h:4`) | `SpaceLabel` | `crate::space_manager` | |
| `struct space_manager` (`space_manager.h:10`) | `SpaceManager` | `crate::space_manager` | |
| `struct table` (`misc/hashtable.h:16`) | `Table<K, V>` | `crate::misc::table` | |
| `struct time_block` (`misc/timer.h:95`) | **not translated** | — | dead code |
| `struct token` (`message.c:254`) | `Token` | `crate::message` | `(start, length)` (`DECISIONS.md` 27) |
| `struct token_value` (`message.c:270`) | `TokenValue` | `crate::message` | |
| `struct ts_buf_hdr` (`misc/sbuffer.h:34`) | **gone** | — | `Vec`/`String` (`DECISIONS.md` 17) |
| `struct view` (`view.h:201`) | `View` | `crate::view` | owns the node arena (`DECISIONS.md` 15) |
| `struct window` (`window.h:87`) | `Window` | `crate::window` | |
| `struct window_animation` (`view.h:68`) | `WindowAnimation` | `crate::view` | |
| `struct window_animation_context` (`view.h:78`) | `AnimationContext`, held as `Arc<AnimationContext>` | `crate::view` | `DECISIONS.md` 24 names this type |
| `struct window_capture` (`view.h:51`) | `WindowCapture` | `crate::view` | |
| `struct window_manager` (`window_manager.h:74`) | `WindowManager` | `crate::window_manager` | |
| `struct window_node` (`view.h:152`) | `WindowNode` | `crate::view` | |
| `struct window_proxy` (`view.h:57`) | `WindowProxy` | `crate::view` | |
| anonymous `g_temp_storage` (`misc/ts.h:4`) | **gone** | — | `DECISIONS.md` 17 |
| anonymous `g_profiler` (`misc/timer.h:16`) | **not translated** | — | dead code |
| anonymous `g_message_loop` (`message.c:1`) | `MessageLoop` | `crate::message` | |
| anonymous `g_mission_control_observer` (`mission_control.c:46`) | `MissionControlObserver` | `crate::mission_control` | |
| `@interface workspace_context` (`workspace.h:21`) | `WorkspaceContext` | `crate::workspace` | |
| `@interface NotifyDelegate` (`misc/notify.h:10`) | `NotifyDelegate` | `crate::misc::notify` | |

### 2.2 Enums

| C | Rust | note |
| --- | --- | --- |
| `enum animation_easing_type` (`misc/helpers.h:27`) | `AnimationEasingType`, `#[repr(usize)]` | 21 variants; `EASING_TYPE_COUNT` is a `const`, not a variant |
| `enum display_arrangement_order` (`display_manager.h:8`) | `DisplayArrangementOrder`, `#[repr(usize)]` | |
| `enum display_property` (`display.h:16`) | **not an enum** — `u64` consts | §6.6 of idioms |
| `enum event_type` (`event_loop.h:48`) | `Event` — variants own their payloads | `DECISIONS.md` 19 |
| `enum external_bar_mode` (`display_manager.h:22`) | `ExternalBarMode`, `#[repr(i32)]` | |
| `enum ffm_mode` (`window_manager.h:40`) | `FfmMode`, `#[repr(i32)]` | |
| `enum label_type` (`message.c:508`) | `LabelType` | |
| `enum mission_control_mode` (`mission_control.c:29`) | `MissionControlMode`, `#[repr(i32)]` | defined in `crate::mission_control`, imported by `crate::state` |
| `enum mouse_drop_action` (`mouse_handler.h:23`) | `MouseDropAction`, `#[repr(i32)]` | |
| `enum mouse_mod` (`mouse_handler.h:34`) | newtype `MouseMod(pub u8)` | OR-ed and index |
| `enum mouse_mode` (`mouse_handler.h:44`) | `MouseMode`, `#[repr(u8)]` | |
| `enum purify_mode` (`window_manager.h:26`) | `PurifyMode`, `#[repr(i32)]` | |
| `enum rule_effects_flag` (`rule.h:22`) | newtype `RuleEffectsFlag(pub u16)` | |
| `enum rule_flag` (`rule.h:8`) | newtype `RuleFlag(pub u16)` | |
| `enum sa_opcode` (`osax/common.h:25`) | `SaOpcode`, `#[repr(u8)]` | generated into `OUT_DIR/osax_common.rs` |
| `enum signal_type` (`event_signal.h:4`) | `SignalType`, `#[repr(u32)]`, explicit `0..=29` | `SIGNAL_TYPE_COUNT` is a `const usize`, not a variant |
| `enum space_op_error` (`space_manager.h:32`) | `SpaceOpError` | explicit `0..=10` as in C |
| `enum space_property` (`view.h:21`) | **not an enum** — `u64` consts | |
| `enum token_type` (`message.c:260`) | `TokenType` — carries the value, replacing the union | §3.28 |
| `enum view_flag` (`view.h:185`) | newtype `ViewFlag(pub u64)` | |
| `enum view_type` (`view.h:169`) | `ViewType`, `#[repr(i32)]` | |
| `enum window_flag` (`window.h:108`) | newtype `WindowFlag(pub u8)` | |
| `enum window_insertion_point` (`view.h:94`) | `WindowInsertionPoint`, `#[repr(u32)]` | |
| `enum window_node_child` (`view.h:108`) | `WindowNodeChild`, `#[repr(i32)]` | |
| `enum window_node_split` (`view.h:122`) | `WindowNodeSplit`, `#[repr(u32)]` | |
| `enum window_op_error` (`window_manager.h:8`) | `WindowOpError` | no explicit discriminants |
| `enum window_origin_mode` (`window_manager.h:54`) | `WindowOriginMode`, `#[repr(i32)]` | |
| `enum window_property` (`window.h:66`) | **not an enum** — `u64` consts | |
| `enum window_rule_flag` (`window.h:120`) | newtype `WindowRuleFlag(pub u8)` | |

### 2.3 Unions

| C | Rust |
| --- | --- |
| anonymous union in `struct token_value` (`message.c:274-279`) | folded into `enum TokenType` variants that carry the value |
| anonymous union in `struct selector` (`message.c:657-662`) | folded into `SelectorOutcome<Target>::Resolved(Target)` |

### 2.4 Typedefs

| C | Rust |
| --- | --- |
| `observer_callback` (`application.h:5`) | `type ObserverCallback` |
| `display_callback` (`display.h:5`) | `type DisplayCallback` |
| `process_event_handler` (`process_manager.h:5`) | `type ProcessEventHandler` |
| `connection_callback` (`misc/extern.h:2`) | `type ConnectionCallback` |
| `table_hash_func` (`misc/hashtable.h:5`) | `type TableHashFunc` — becomes the `hash: fn(&K) -> u64` field |
| `table_compare_func` (`misc/hashtable.h:8`) | `type TableCompareFunc` — **vanishes**, replaced by `K: PartialEq` |

### 2.5 Rust types with no C struct behind them

| Rust type | module | why it exists |
| --- | --- | --- |
| `EventLoopOwnedState` | `crate::state` | `DECISIONS.md` 12 |
| `ProcessManager` | `crate::process_manager` | the event-loop-visible remainder of `struct process_manager` |
| `CarbonProcessEventInstallation` | `crate::process_manager` | keeps the Carbon UPP alive; `target`/`handler`/`type`/`ref` |
| `MouseDragState` | `crate::state` | event-loop half of `struct mouse_state` (`DECISIONS.md` 23) |
| `MouseTapState` | `crate::mouse_handler` | atomic half of `struct mouse_state` |
| `WindowLivenessCell` | `crate::window` | `DECISIONS.md` 21 |
| `PosixRegex` | `crate::misc::regex` | `Drop` wrapper over `regex_t` (`DECISIONS.md` 26) |
| `RegexMatch` | `crate::misc::regex` | the three-valued match result |
| `SignalProp` | `crate::event_signal` | `SIGNAL_PROP_UD/YES/NO` as an enum |
| `Response` | `crate::misc::response` | `DECISIONS.md` 28 |
| `MessageCursor<'message>` | `crate::message` | `DECISIONS.md` 27 |
| `CFStringOwned` | `crate::ffi` | owned `CFStringRef` with `Drop` |
| `OsaxPaths` | `crate::sa` | the eleven `sa.m` path buffers |
| `MessageLoop` | `crate::message` | `UnixListener` + accept `JoinHandle` |
| `MissionControlObserver` | `crate::mission_control` | the two CF refs |
| `MacosVersion` | `crate::workspace` | the six version flags |
| `ResizeHandle` | `crate::misc::macros` | `HANDLE_*` |
| `OsaxAttrib` | generated `osax_common.rs` | `OSAX_ATTRIB_*` |
| `AxWindowNotification` | `crate::window` | `AX_WINDOW_*` |
| `AxApplicationNotification` | `crate::application` | `AX_APPLICATION_*` |

---

## 3. Struct fields

One row per C field. The Rust column is the **only** spelling. Pointer fields follow
`patterns/state-and-ownership.md` §3.2 and say which handle they become.

### 3.1 `struct window_manager` → `WindowManager` (`window_manager.h:74-103`)

| C field | C type | Rust field | Rust type |
| --- | --- | --- | --- |
| `system_element` | `AXUIElementRef` | `system_element` | `AXUIElementRef` |
| `application` | `struct table` | `application` | `Table<ProcessId, Application>` — owns the value |
| `window` | `struct table` | `window` | `Table<WindowId, Window>` — owns the value |
| `managed_window` | `struct table` | `managed_window` | `Table<WindowId, SpaceId>` — **handle**, was `struct view *` |
| `window_lost_focused_event` | `struct table` | `window_lost_focused_event` | `Table<WindowId, ()>` — a set; C stored `(void *)1` |
| `application_lost_front_switched_event` | `struct table` | `application_lost_front_switched_event` | `Table<ProcessId, ()>` — a set |
| `window_animations_table` | `struct table` | `window_animations_table` | `Arc<Mutex<Table<WindowId, (Arc<AnimationContext>, usize)>>>` — **handle**, was an interior pointer |
| `insert_feedback` | `struct table` | `insert_feedback` | `Table<WindowId, (SpaceId, NodeId)>` — **handle**, was `struct window_node *`; key stays `node.window_order[0]` |
| `window_animations_lock` | `pthread_mutex_t` | **gone** | absorbed into the `Mutex` above (`DECISIONS.md` 24) |
| `rules` | `struct rule *` (sbuffer) | `rules` | `Vec<Rule>` |
| `applications_to_refresh` | `struct application **` (sbuffer) | `applications_to_refresh` | `Vec<ProcessId>` — **handle** |
| `focused_window_id` | `uint32_t` | `focused_window_id` | `WindowId` |
| `focused_window_psn` | `ProcessSerialNumber` | `focused_window_process_serial_number` | `ProcessSerialNumber` |
| `last_window_id` | `uint32_t` | `last_window_id` | `WindowId` |
| `enable_mff` | `bool` | `enable_mff` | `bool` — `mff` is the config word, kept |
| `ffm_mode` | `enum ffm_mode` | `ffm_mode` | `FfmMode` |
| `purify_mode` | `enum purify_mode` | `purify_mode` | `PurifyMode` |
| `window_origin_mode` | `enum window_origin_mode` | `window_origin_mode` | `WindowOriginMode` |
| `enable_window_opacity` | `bool` | `enable_window_opacity` | `bool` |
| `menubar_opacity` | `float` | `menubar_opacity` | `f32` |
| `active_window_opacity` | `float` | `active_window_opacity` | `f32` |
| `normal_window_opacity` | `float` | `normal_window_opacity` | `f32` |
| `window_opacity_duration` | `float` | `window_opacity_duration` | `f32` |
| `window_animation_duration` | `float` | `window_animation_duration` | `f32` |
| `window_animation_easing` | `int` | `window_animation_easing` | `AnimationEasingType` |
| `insert_feedback_color` | `struct rgba_color` | `insert_feedback_color` | `RgbaColor` |
| `scratchpad_window` | `struct scratchpad *` (sbuffer) | `scratchpad_window` | `Vec<Scratchpad>` |

### 3.2 `struct window` → `Window` (`window.h:87-106`)

| C field | C type | Rust field | Rust type |
| --- | --- | --- | --- |
| `application` | `struct application *` | `application` | `Option<ProcessId>` — **handle**; `None` models `event_loop.c:281` |
| `ref` | `AXUIElementRef` | `element_ref` | `AXUIElementRef` — mandatory rename, `ref` is a Rust keyword |
| `id` | `uint32_t` | `id` | `WindowId` |
| `id_ptr` | `uint32_t *volatile` | `liveness` | `Arc<WindowLivenessCell>` (`DECISIONS.md` 21) |
| — | — | `liveness_reference_held_by_the_observation` | `Option<*const WindowLivenessCell>` — the one strong count `window_observe` mints |
| `role` | `CFStringRef` | `role` | `Option<CFStringOwned>` |
| `subrole` | `CFStringRef` | `subrole` | `Option<CFStringOwned>` |
| `title` | `CFStringRef` | `title` | `Option<CFStringOwned>` |
| `frame` | `CGRect` | `frame` | `CGRect` |
| `windowed_frame` | `CGRect` | `windowed_frame` | `CGRect` |
| `is_root` | `bool` | `is_root` | `bool` |
| `is_eligible` | `bool` | `is_eligible` | `bool` |
| `notification` | `uint8_t` | `notification` | `u8`, tested with `AxWindowNotification` |
| `rule_flags` | `uint8_t` | `rule_flags` | `u8`, tested with `WindowRuleFlag` |
| `flags` | `uint8_t` | `flags` | `u8`, tested with `WindowFlag` |
| `opacity` | `float` | `opacity` | `f32` |
| `layer` | `int` | `layer` | `i32` |
| `scratchpad` | `char *` (aliases the label) | `scratchpad` | `Option<String>` — a **clone** of `Scratchpad::label` |

### 3.3 `struct application` → `Application` (`application.h:68-80`)

| C field | C type | Rust field | Rust type |
| --- | --- | --- | --- |
| `ref` | `AXUIElementRef` | `element_ref` | `AXUIElementRef` |
| `connection` | `int` | `connection` | `i32` |
| `psn` | `ProcessSerialNumber` | `process_serial_number` | `ProcessSerialNumber` |
| `pid` | `pid_t` | `process_id` | `ProcessId` |
| `name` | `char *` (aliases `process->name`) | `name` | `Arc<str>`, cloned from `Process::name` |
| `observer_ref` | `AXObserverRef` | `observer_ref` | `AXObserverRef` |
| `notification` | `uint8_t` | `notification` | `u8`, tested with `AxApplicationNotification` |
| `is_observing` | `bool` | `is_observing` | `bool` |
| `is_hidden` | `bool` | `is_hidden` | `bool` |
| `ax_retry` | `bool` | `ax_retry` | `bool` |

### 3.4 `struct process` → `Process`, held as `Arc<Process>` (`process_manager.h:7-15`)

| C field | C type | Rust field | Rust type |
| --- | --- | --- | --- |
| `psn` | `ProcessSerialNumber` | `process_serial_number` | `ProcessSerialNumber` |
| `pid` | `pid_t` | `process_id` | `ProcessId` |
| `name` | `char *` | `name` | `Arc<str>` — owned; a NULL `cfstring_copy` makes `process_create` return `None` |
| `ns_application` | `void *` | `ns_application` | `AtomicPtr<c_void>` (`DECISIONS.md` 22) |
| `policy` | `int` | `policy` | `AtomicI32` — explicitly initialised; C leaves it uninitialised (one `DEVIATIONS.md` line) |
| `terminated` | `bool volatile` | `terminated` | `AtomicBool` (`DECISIONS.md` 22) |

### 3.5 `struct process_manager` → split three ways (`process_manager.h:17-28`)

| C field | C type | Rust | note |
| --- | --- | --- | --- |
| `process` | `struct table` | `static PROCESS_TABLE: Mutex<Table<ProcessSerialNumber, Arc<Process>>>` | `DECISIONS.md` 22 |
| `target` | `EventTargetRef` | `CarbonProcessEventInstallation::target` | inside `static CARBON_PROCESS_EVENT_INSTALLATION: OnceLock<_>` |
| `handler` | `EventHandlerUPP` | `CarbonProcessEventInstallation::handler` | |
| `type[3]` | `EventTypeSpec[3]` | `CarbonProcessEventInstallation::event_type` | `[EventTypeSpec; 3]`, `#[repr(C)]` |
| `ref` | `EventHandlerRef` | `CarbonProcessEventInstallation::handler_ref` | `ref` is a Rust keyword |
| `front_pid` | `pid_t` | `ProcessManager::front_process_id` | `ProcessId` |
| `last_front_pid` | `pid_t` | `ProcessManager::last_front_process_id` | `ProcessId` |
| `switch_event_time` | `EventTime` (`double`) | `ProcessManager::switch_event_time` | `f64` |
| `finder_psn` | `ProcessSerialNumber` | `ProcessManager::finder_process_serial_number` | `ProcessSerialNumber` |

### 3.6 `struct space_manager` → `SpaceManager` (`space_manager.h:10-30`)

| C field | C type | Rust field | Rust type |
| --- | --- | --- | --- |
| `view` | `struct table` | `view` | `Table<SpaceId, View>` — owns the `View` by value |
| `current_space_id` | `uint64_t` | `current_space_id` | `SpaceId` |
| `last_space_id` | `uint64_t` | `last_space_id` | `SpaceId` |
| `did_begin` | `bool` | `did_begin` | `bool` |
| `layout` | `enum view_type` | `layout` | `ViewType` |
| `top_padding` | `int` | `top_padding` | `i32` |
| `bottom_padding` | `int` | `bottom_padding` | `i32` |
| `left_padding` | `int` | `left_padding` | `i32` |
| `right_padding` | `int` | `right_padding` | `i32` |
| `window_gap` | `int` | `window_gap` | `i32` |
| `split_ratio` | `float` | `split_ratio` | `f32` |
| `split_type` | `enum window_node_split` | `split_type` | `WindowNodeSplit` |
| `window_placement` | `enum window_node_child` | `window_placement` | `WindowNodeChild` |
| `window_insertion_point` | `enum window_insertion_point` | `window_insertion_point` | `WindowInsertionPoint` |
| `window_zoom_persist` | `bool` | `window_zoom_persist` | `bool` |
| `auto_balance` | `uint32_t` | `auto_balance` | `u32` — holds a `WindowNodeSplit` discriminant |
| `labels` | `struct space_label *` (sbuffer) | `labels` | `Vec<SpaceLabel>` |
| `skip_window_focus_animation` | `bool` | `skip_window_focus_animation` | `bool` |

### 3.7 `struct space_label` / `struct display_label`

| C | Rust field | Rust type |
| --- | --- | --- |
| `space_label::sid` (`space_manager.h:6`) | `space_id` | `SpaceId` |
| `space_label::label` (`:7`) | `label` | `String` — owned |
| `display_label::did` (`display_manager.h:38`) | `display_id` | `DisplayId` |
| `display_label::label` (`:39`) | `label` | `String` — owned |

### 3.8 `struct display_manager` → `DisplayManager` (`display_manager.h:42-54`)

| C field | C type | Rust field | Rust type |
| --- | --- | --- | --- |
| `current_display_id` | `uint32_t` | `current_display_id` | `DisplayId` |
| `last_display_id` | `uint32_t` | `last_display_id` | `DisplayId` |
| `top_padding` | `int` | `top_padding` | `i32` |
| `bottom_padding` | `int` | `bottom_padding` | `i32` |
| `order` | `enum display_arrangement_order` | `order` | `DisplayArrangementOrder` |
| `mode` | `enum external_bar_mode` | `mode` | `ExternalBarMode` |
| `labels` | `struct display_label *` (sbuffer) | `labels` | `Vec<DisplayLabel>` |

### 3.9 `struct view` → `View` (`view.h:201-216`)

| C field | C type | Rust field | Rust type |
| --- | --- | --- | --- |
| `uuid` | `CFStringRef` | `uuid` | `Option<CFStringOwned>` |
| `sid` | `uint64_t` | `space_id` | `SpaceId` |
| `root` | `struct window_node *` | **gone** | the root is always `ROOT_NODE_ID` (`DECISIONS.md` 15) |
| — | — | `nodes` | `Vec<Option<WindowNode>>` — the arena |
| — | — | `free_node_ids` | `Vec<NodeId>` |
| `insertion_point` | `uint32_t` | `insertion_point` | `WindowId` — this is a **window id**, not a `WindowInsertionPoint` |
| `layout` | `enum view_type` | `layout` | `ViewType` |
| `split_type` | `enum window_node_split` | `split_type` | `WindowNodeSplit` |
| `top_padding` | `int` | `top_padding` | `i32` |
| `bottom_padding` | `int` | `bottom_padding` | `i32` |
| `left_padding` | `int` | `left_padding` | `i32` |
| `right_padding` | `int` | `right_padding` | `i32` |
| `window_gap` | `int` | `window_gap` | `i32` |
| `auto_balance` | `uint32_t` | `auto_balance` | `u32` |
| `flags` | `uint64_t` | `flags` | `u64`, tested with `ViewFlag` |

The four arena accessors, and only these four: `View::node`, `View::node_mut`,
`View::find_node`, `View::find_node_mut`.

### 3.10 `struct window_node` → `WindowNode` (`view.h:152-167`)

| C field | C type | Rust field | Rust type |
| --- | --- | --- | --- |
| `area` | `struct area` | `area` | `Area` |
| `parent` | `struct window_node *` | `parent` | `Option<NodeId>` |
| `left` | `struct window_node *` | `left` | `Option<NodeId>` |
| `right` | `struct window_node *` | `right` | `Option<NodeId>` |
| `zoom` | `struct window_node *` | `zoom` | `Option<NodeId>` |
| `window_list[32]` | `uint32_t[]` | `window_list` | `[WindowId; NODE_MAX_WINDOW_COUNT]` |
| `window_order[32]` | `uint32_t[]` | `window_order` | `[WindowId; NODE_MAX_WINDOW_COUNT]` |
| `window_count` | `int` | `window_count` | `i32` — stays `i32`; index sites cast `as usize` |
| `ratio` | `float` | `ratio` | `f32` |
| `split` | `enum window_node_split` | `split` | `WindowNodeSplit` |
| `child` | `enum window_node_child` | `child` | `WindowNodeChild` |
| `insert_dir` | `int` | `insert_direction` | `i32` — holds a `DIR_*` constant; `0` is the live "none" sentinel |
| `feedback_window` | `struct feedback_window` | `feedback_window` | `Option<FeedbackWindow>` |

### 3.11 `struct area` → `Area` (`view.h:42-48`)

| C field | Rust field | Rust type |
| --- | --- | --- |
| `x` | `x` | `f32` |
| `y` | `y` | `f32` |
| `w` | `width` | `f32` |
| `h` | `height` | `f32` |

### 3.12 `struct window_capture` → `WindowCapture` (`view.h:51-55`)

| C field | Rust field | Rust type |
| --- | --- | --- |
| `window` (`struct window *`) | `window_id` | `WindowId` — **handle** |
| `x` | `x` | `f32` |
| `y` | `y` | `f32` |
| `w` | `width` | `f32` |
| `h` | `height` | `f32` |

### 3.13 `struct window_proxy` → `WindowProxy` (`view.h:57-66`)

| C field | Rust field | Rust type |
| --- | --- | --- |
| `id` | `id` | `WindowId` — the SkyLight proxy window |
| `context` | `context` | `CGContextRef` |
| `tx` | `target_x` | `AtomicU32` holding `f32` bits (`DECISIONS.md` 24) |
| `ty` | `target_y` | `AtomicU32` holding `f32` bits |
| `tw` | `target_width` | `AtomicU32` holding `f32` bits |
| `th` | `target_height` | `AtomicU32` holding `f32` bits |
| `frame` | `frame` | `CGRect` |
| `level` | `level` | `i32` |
| `sub_level` | `sub_level` | `i32` |
| `image` | `image` | `Option<CGImageRef>` |

### 3.14 `struct window_animation` → `WindowAnimation` (`view.h:68-76`)

| C field | Rust field | Rust type |
| --- | --- | --- |
| `window` (`struct window *`) **and** `wid` | `window_id` | `WindowId` — the two C fields hold the same window and **collapse into one**; one `DEVIATIONS.md` line |
| `x` | `x` | `f32` |
| `y` | `y` | `f32` |
| `w` | `width` | `f32` |
| `h` | `height` | `f32` |
| `cid` | `connection_id` | `i32` |
| `proxy` | `proxy` | `WindowProxy` |
| `skip` | `skip` | `AtomicBool` (`DECISIONS.md` 24) |

### 3.15 `struct window_animation_context` → `AnimationContext` (`view.h:78-86`)

| C field | Rust field | Rust type |
| --- | --- | --- |
| `animation_connection` | `animation_connection` | `i32` |
| `animation_easing` | `animation_easing` | `AnimationEasingType` |
| `animation_duration` | `animation_duration` | `f32` |
| `animation_clock` | `animation_clock` | `AtomicU64` |
| `animation_list` | `animation_list` | `Vec<WindowAnimation>` |
| `animation_count` | `animation_count` | `i32` |

### 3.16 `struct balance_node` → `BalanceNode` (`view.h:88-92`)

| C field | Rust field | Rust type |
| --- | --- | --- |
| `y_count` | `y_count` | `i32` |
| `x_count` | `x_count` | `i32` |

### 3.17 `struct feedback_window` → `FeedbackWindow` (`view.h:145-149`)

| C field | Rust field | Rust type |
| --- | --- | --- |
| `id` | `id` | `WindowId` |
| `context` | `context` | `CGContextRef` |

### 3.18 `struct scratchpad` → `Scratchpad` (`window_manager.h:68-72`)

| C field | Rust field | Rust type |
| --- | --- | --- |
| `label` | `label` | `String` — owned |
| `window` (`struct window *`) | `window_id` | `WindowId` — **handle** |

### 3.19 `struct rule` → `Rule` (`rule.h:44-57`)

| C field | Rust field | Rust type |
| --- | --- | --- |
| `label` | `label` | `Option<String>` |
| `app` | `app` | `Option<String>` — the C config word, kept |
| `title` | `title` | `Option<String>` |
| `role` | `role` | `Option<String>` |
| `subrole` | `subrole` | `Option<String>` |
| `app_regex` + `RULE_APP_VALID` | `app_regex` | `Option<PosixRegex>` |
| `title_regex` + `RULE_TITLE_VALID` | `title_regex` | `Option<PosixRegex>` |
| `role_regex` + `RULE_ROLE_VALID` | `role_regex` | `Option<PosixRegex>` |
| `subrole_regex` + `RULE_SUBROLE_VALID` | `subrole_regex` | `Option<PosixRegex>` |
| `effects` | `effects` | `RuleEffects` |
| `flags` | `flags` | `u16`, tested with `RuleFlag` |

### 3.20 `struct rule_effects` → `RuleEffects` (`rule.h:29-42`)

| C field | Rust field | Rust type |
| --- | --- | --- |
| `did` | `display_id` | `DisplayId` |
| `sid` | `space_id` | `SpaceId` |
| `opacity` | `opacity` | `f32` |
| `manage` | `manage` | `i32` — `RULE_PROP_UD/ON/OFF` |
| `sticky` | `sticky` | `i32` |
| `mff` | `mff` | `i32` |
| `layer` | `layer` | `i32` |
| `fullscreen` | `fullscreen` | `i32` |
| `grid[6]` | `grid` | `[u32; 6]` |
| `scratchpad` | `scratchpad` | `Option<String>` |
| `flags` | `flags` | `u16`, tested with `RuleEffectsFlag` |

### 3.21 `struct signal` → `Signal` (`event_signal.h:104-117`)

| C field | Rust field | Rust type |
| --- | --- | --- |
| `app` | `app` | `Option<String>` |
| `title` | `title` | `Option<String>` |
| `app_regex_valid` | **gone** | folded into `Option<PosixRegex>` |
| `title_regex_valid` | **gone** | folded into `Option<PosixRegex>` |
| `app_regex_exclude` | `app_regex_exclude` | `bool` |
| `title_regex_exclude` | `title_regex_exclude` | `bool` |
| `app_regex` | `app_regex` | `Option<PosixRegex>` |
| `title_regex` | `title_regex` | `Option<PosixRegex>` |
| `active` | `active` | `SignalProp` |
| `command` | `command` | `Option<String>` |
| `label` | `label` | `Option<String>` |

### 3.22 `struct event_signal` → `PendingSignal` (`event_signal.h:94-102`)

| C field | Rust field | Rust type |
| --- | --- | --- |
| `type` | `signal_type` | `SignalType` — `type` is a Rust keyword and this is not a type discriminator |
| `arg_name[4]` + `arg_value[4]` | `arguments` | `[Option<(String, String)>; 4]` — the two C arrays are one array of owned pairs |
| `app` | `app` | `Option<String>` — owned, `to_string()` at `event_signal.c:146` |
| `title` | `title` | `Option<String>` — owned, `to_string()` at `event_signal.c:209` |
| `active` | `active` | `i32` — holds 0 or 1, compared against `SignalProp` |

### 3.23 `struct mouse_state` → `MouseTapState` + `MouseDragState` (`mouse_handler.h:62-81`)

`DECISIONS.md` 23. `static MOUSE_TAP_STATE: MouseTapState` is process-wide; `MouseDragState` is a
field of `EventLoopOwnedState` named `mouse_drag_state`.

| C field | Rust owner | Rust field | Rust type |
| --- | --- | --- | --- |
| `handle` | `MouseTapState` | `handle` | `AtomicPtr<__CFMachPort>` |
| `runloop_source` | `MouseTapState` | `runloop_source` | `AtomicPtr<__CFRunLoopSource>` |
| `consume_mouse_click` | `MouseTapState` | `consume_mouse_click` | `AtomicBool` |
| `drag_detected` | `MouseTapState` | `drag_detected` | `AtomicBool` |
| `consumed_event` | `MouseTapState` | `consumed_event` | `AtomicPtr<CGEvent>` |
| `modifier` | `MouseTapState` | `modifier` | `AtomicU8` holding a `MouseMod` |
| `action1` | `MouseTapState` | `action1` | `AtomicU8` holding a `MouseMode` |
| `action2` | `MouseTapState` | `action2` | `AtomicU8` holding a `MouseMode` |
| `drop_action` | `MouseTapState` | `drop_action` | `AtomicU8` holding a `MouseMode` |
| `current_action` | `MouseDragState` | `current_action` | `MouseMode` |
| `down_location` | `MouseDragState` | `down_location` | `CGPoint` |
| `last_moved_time` | `MouseDragState` | `last_moved_time` | `u64` |
| `window` (`struct window *`) | `MouseDragState` | `window_id` | `Option<WindowId>` — **handle** |
| `window_frame` | `MouseDragState` | `window_frame` | `CGRect` |
| `ffm_window_id` | `MouseDragState` | `ffm_window_id` | `WindowId` |
| `direction` | `MouseDragState` | `direction` | `u8`, tested with `ResizeHandle` |
| `feedback_node` (`struct window_node *`) | `MouseDragState` | `feedback_node` | `Option<(SpaceId, NodeId)>` — **handle**, cleared when the node's slot is freed |

`action1`/`action2` keep the C digits: they are the two configured mouse actions, not a pair that
wants naming.

### 3.24 `struct mouse_window_info` → `MouseWindowInfo` (`mouse_handler.h:53-60`)

| C field | Rust field | Rust type |
| --- | --- | --- |
| `dx` | `delta_x` | `f32` |
| `dy` | `delta_y` | `f32` |
| `dw` | `delta_width` | `f32` |
| `dh` | `delta_height` | `f32` |
| `changed_x` | `changed_x` | `bool` |
| `changed_y` | `changed_y` | `bool` |
| `changed_w` | `changed_width` | `bool` |
| `changed_h` | `changed_height` | `bool` |
| `changed_position` | `changed_position` | `bool` |
| `changed_size` | `changed_size` | `bool` |

### 3.25 `struct event` and `struct event_loop` (`event_loop.h:55-71`)

| C field | Rust |
| --- | --- |
| `event::type` | the `Event` variant itself |
| `event::param1` | a named payload field on the variant |
| `event::context` | a named, owned payload field on the variant |
| `event::next` | **gone** — `mpsc` owns the queue |
| `event_loop::is_running` | **gone** — `for event in receiver` |
| `event_loop::thread` | **gone** — the `JoinHandle` of `std::thread::spawn` |
| `event_loop::semaphore` | **gone** |
| `event_loop::pool` | **gone** |
| `event_loop::head` / `tail` | **gone** |

### 3.26 `struct table` → `Table<K, V>` (`misc/hashtable.h:16-24`)

| C field | Rust field | Rust type |
| --- | --- | --- |
| `count` | `count` | `i32` |
| `capacity` | `capacity` | `i32` |
| `max_load` | `max_load` | `f32` (`0.75`) |
| `hash` | `hash` | `fn(&K) -> u64` |
| `cmp` | **gone** | replaced by `K: PartialEq` |
| `buckets` | `buckets` | `Vec<Vec<(K, V)>>` — append at tail, `Vec::remove` on delete, **never** `swap_remove` |
| `bucket::key` / `value` / `next` | **gone** | the `(K, V)` tuple and the `Vec` |

### 3.27 Replaced allocators (`DECISIONS.md` 17)

| C field | site | Rust |
| --- | --- | --- |
| `buf_hdr::len` | `misc/sbuffer.h:6` | `Vec::len` |
| `buf_hdr::cap` | `:7` | `Vec::capacity` |
| `buf_hdr::buf` | `:8` | the `Vec` itself |
| `ts_buf_hdr::len` / `cap` / `buf` | `:36-38` | `Vec`/`String` |
| `memory_pool::memory` / `size` / `used` | `misc/memory_pool.h:6-8` | an owned queue |
| `g_temp_storage::memory` / `size` / `used` | `misc/ts.h:5-7` | `Vec`/`String` |

If a local ever needs the C words: `len` → `length`, `cap` → `capacity`, `buf` → `buffer`,
`ptr` → `pointer`, `mod` (the `ts_align` remainder) → `misalignment`.

### 3.28 Message parsing types (`message.c`)

| C field | Rust field | Rust type |
| --- | --- | --- |
| `token::text` | `start` | `usize` — an index into the message buffer, not a pointer |
| `token::length` | `length` | `usize` |
| `token_value::token` | `token` | `Token` |
| `token_value::type` | `type_of_value` | `TokenType` — `type` is a Rust keyword |
| `token_value::int_value` | `TokenType::Int(i32)` | payload of the variant |
| `token_value::float_value` | `TokenType::Float(f32)` | |
| `token_value::u32_value` | `TokenType::U32(u32)` | |
| `token_value::string_value` | **gone** | read from `value.token` and the buffer |
| `properties::token` | `token` | `Token` |
| `properties::did_parse` | `did_parse` | `bool` |
| `properties::did_error` | `did_error` | `bool` |
| `properties::flags` | `flags` | `u64` |
| `selector::token` | `token` | `Token` |
| `selector::did_parse` | `did_parse()` | method over `SelectorOutcome` |
| `selector::dir` | `Selector<i32>` | `SelectorOutcome::Resolved(direction)` |
| `selector::did` | `Selector<DisplayId>` | |
| `selector::sid` | `Selector<SpaceId>` | |
| `selector::window` | `Selector<WindowId>` | a **handle**, never a reference |

`MessageCursor<'message>` carries `bytes: &'message mut [u8]` and `at: usize`.

### 3.29 File-scope aggregates

| C | Rust field | Rust type |
| --- | --- | --- |
| `g_message_loop::sockfd` (`message.c:2`) | `MessageLoop::listener` | `UnixListener` |
| `g_message_loop::is_running` (`:3`) | **gone** | `for stream in listener.incoming()` |
| `g_message_loop::thread` (`:4`) | `MessageLoop::thread` | `JoinHandle<()>` |
| `g_mission_control_observer::ref` (`mission_control.c:47`) | `MissionControlObserver::element_ref` | `AXUIElementRef` |
| `g_mission_control_observer::observer_ref` (`:48`) | `MissionControlObserver::observer_ref` | `AXObserverRef` |
| `g_mission_control_observer::is_observing` (`:49`) | **gone** | `Option::is_some` on the `Mutex<Option<MissionControlObserver>>` |
| `osax_base_dir` (`sa.m:9`) | `OsaxPaths::base_directory` | `String` |
| `osax_contents_dir` (`:10`) | `OsaxPaths::contents_directory` | `String` |
| `osax_contents_macos_dir` (`:11`) | `OsaxPaths::contents_macos_directory` | `String` |
| `osax_contents_res_dir` (`:12`) | `OsaxPaths::contents_resources_directory` | `String` |
| `osax_info_plist` (`:13`) | `OsaxPaths::info_plist` | `String` |
| `osax_payload_dir` (`:14`) | `OsaxPaths::payload_directory` | `String` |
| `osax_payload_contents_dir` (`:15`) | `OsaxPaths::payload_contents_directory` | `String` |
| `osax_payload_contents_macos_dir` (`:16`) | `OsaxPaths::payload_contents_macos_directory` | `String` |
| `osax_payload_plist` (`:17`) | `OsaxPaths::payload_plist` | `String` |
| `osax_bin_payload` (`:18`) | `OsaxPaths::binary_payload` | `String` |
| `osax_bin_loader` (`:19`) | `OsaxPaths::binary_loader` | `String` |

### 3.30 `struct rgba_color` → `RgbaColor` (`misc/helpers.h:162-169`)

| C field | Rust field | Rust type |
| --- | --- | --- |
| `p` | `packed` | `u32` — the hex the user typed, echoed back at `message.c:1373` |
| `r` | `red` | `f32` |
| `g` | `green` | `f32` |
| `b` | `blue` | `f32` |
| `a` | `alpha` | `f32` |

### 3.31 `EventLoopOwnedState` (`crate::state`)

Field order is the `src/yabai.c:27-35` declaration order. **Not** a parameter order.

| Rust field | Rust type | from |
| --- | --- | --- |
| `signal_event` | `[Vec<Signal>; SIGNAL_TYPE_COUNT]` | `g_signal_event`, `yabai.c:27` |
| `process_manager` | `ProcessManager` | `g_process_manager`, `:28` |
| `display_manager` | `DisplayManager` | `g_display_manager`, `:29` |
| `window_manager` | `WindowManager` | `g_window_manager`, `:30` |
| `space_manager` | `SpaceManager` | `g_space_manager`, `:31` |
| `signal_storage` | `Vec<PendingSignal>` | `g_signal_storage`, `:32` |
| `mouse_drag_state` | `MouseDragState` | `g_mouse_state`, `:33` |
| `mission_control_mode` | `MissionControlMode` | `g_mission_control_mode`, `:37` |
| `focus_follows_mouse_suspended_value` | `FfmMode` | `ffm_value`, `event_loop.c:1561` |
| `is_menu_open` | `i32` | `is_menu_open`, `event_loop.c:1562` |

### 3.32 `WindowLivenessCell` (`crate::window`)

| Rust field | Rust type |
| --- | --- |
| `window_id` | `WindowId` — write-once |
| `application_process_id` | `ProcessId` — write-once |
| `state` | `AtomicU8` |

---

## 4. Enum variant spellings — every enum constant

### 4.1 `SignalType` — `event_signal.h:4-45`, `#[repr(u32)]`, explicit `0..=29`

| C | Rust | value |
| --- | --- | --- |
| `SIGNAL_TYPE_UNKNOWN` | `SignalType::Unknown` | 0 |
| `SIGNAL_APPLICATION_LAUNCHED` | `SignalType::ApplicationLaunched` | 1 |
| `SIGNAL_APPLICATION_TERMINATED` | `SignalType::ApplicationTerminated` | 2 |
| `SIGNAL_APPLICATION_FRONT_SWITCHED` | `SignalType::ApplicationFrontSwitched` | 3 |
| `SIGNAL_APPLICATION_ACTIVATED` | `SignalType::ApplicationActivated` | 4 |
| `SIGNAL_APPLICATION_DEACTIVATED` | `SignalType::ApplicationDeactivated` | 5 |
| `SIGNAL_APPLICATION_VISIBLE` | `SignalType::ApplicationVisible` | 6 |
| `SIGNAL_APPLICATION_HIDDEN` | `SignalType::ApplicationHidden` | 7 |
| `SIGNAL_WINDOW_CREATED` | `SignalType::WindowCreated` | 8 |
| `SIGNAL_WINDOW_DESTROYED` | `SignalType::WindowDestroyed` | 9 |
| `SIGNAL_WINDOW_FOCUSED` | `SignalType::WindowFocused` | 10 |
| `SIGNAL_WINDOW_MOVED` | `SignalType::WindowMoved` | 11 |
| `SIGNAL_WINDOW_RESIZED` | `SignalType::WindowResized` | 12 |
| `SIGNAL_WINDOW_MINIMIZED` | `SignalType::WindowMinimized` | 13 |
| `SIGNAL_WINDOW_DEMINIMIZED` | `SignalType::WindowDeminimized` | 14 |
| `SIGNAL_WINDOW_TITLE_CHANGED` | `SignalType::WindowTitleChanged` | 15 |
| `SIGNAL_SPACE_CREATED` | `SignalType::SpaceCreated` | 16 |
| `SIGNAL_SPACE_DESTROYED` | `SignalType::SpaceDestroyed` | 17 |
| `SIGNAL_SPACE_CHANGED` | `SignalType::SpaceChanged` | 18 |
| `SIGNAL_DISPLAY_ADDED` | `SignalType::DisplayAdded` | 19 |
| `SIGNAL_DISPLAY_REMOVED` | `SignalType::DisplayRemoved` | 20 |
| `SIGNAL_DISPLAY_MOVED` | `SignalType::DisplayMoved` | 21 |
| `SIGNAL_DISPLAY_RESIZED` | `SignalType::DisplayResized` | 22 |
| `SIGNAL_DISPLAY_CHANGED` | `SignalType::DisplayChanged` | 23 |
| `SIGNAL_MISSION_CONTROL_ENTER` | `SignalType::MissionControlEnter` | 24 |
| `SIGNAL_MISSION_CONTROL_EXIT` | `SignalType::MissionControlExit` | 25 |
| `SIGNAL_DOCK_DID_CHANGE_PREF` | `SignalType::DockDidChangePref` | 26 |
| `SIGNAL_DOCK_DID_RESTART` | `SignalType::DockDidRestart` | 27 |
| `SIGNAL_MENU_BAR_HIDDEN_CHANGED` | `SignalType::MenuBarHiddenChanged` | 28 |
| `SIGNAL_SYSTEM_WOKE` | `SignalType::SystemWoke` | 29 |
| `SIGNAL_TYPE_COUNT` | `pub(crate) const SIGNAL_TYPE_COUNT: usize = 30;` | not a variant |

`DockDidChangePref` keeps `Pref`. Do not expand it to `Preference`: it is the C's own word for a
variant, covered by rule 1.

### 4.2 `Event` — `event_loop.h:6-46`, 40 variants in the C order

| C entry | Rust variant | handler function |
| --- | --- | --- |
| `APPLICATION_LAUNCHED` | `Event::ApplicationLaunched` | `event_handler_application_launched` |
| `APPLICATION_TERMINATED` | `Event::ApplicationTerminated` | `event_handler_application_terminated` |
| `APPLICATION_FRONT_SWITCHED` | `Event::ApplicationFrontSwitched` | `event_handler_application_front_switched` |
| `APPLICATION_VISIBLE` | `Event::ApplicationVisible` | `event_handler_application_visible` |
| `APPLICATION_HIDDEN` | `Event::ApplicationHidden` | `event_handler_application_hidden` |
| `WINDOW_CREATED` | `Event::WindowCreated` | `event_handler_window_created` |
| `WINDOW_DESTROYED` | `Event::WindowDestroyed` | `event_handler_window_destroyed` |
| `WINDOW_FOCUSED` | `Event::WindowFocused` | `event_handler_window_focused` |
| `WINDOW_MOVED` | `Event::WindowMoved` | `event_handler_window_moved` |
| `WINDOW_RESIZED` | `Event::WindowResized` | `event_handler_window_resized` |
| `WINDOW_MINIMIZED` | `Event::WindowMinimized` | `event_handler_window_minimized` |
| `WINDOW_DEMINIMIZED` | `Event::WindowDeminimized` | `event_handler_window_deminimized` |
| `WINDOW_TITLE_CHANGED` | `Event::WindowTitleChanged` | `event_handler_window_title_changed` |
| `SLS_WINDOW_ORDERED` | `Event::SlsWindowOrdered` | `event_handler_sls_window_ordered` |
| `SLS_WINDOW_DESTROYED` | `Event::SlsWindowDestroyed` | `event_handler_sls_window_destroyed` |
| `SLS_SPACE_CREATED` | `Event::SlsSpaceCreated` | `event_handler_sls_space_created` |
| `SLS_SPACE_DESTROYED` | `Event::SlsSpaceDestroyed` | `event_handler_sls_space_destroyed` |
| `SPACE_CHANGED` | `Event::SpaceChanged` | `event_handler_space_changed` |
| `DISPLAY_ADDED` | `Event::DisplayAdded` | `event_handler_display_added` |
| `DISPLAY_REMOVED` | `Event::DisplayRemoved` | `event_handler_display_removed` |
| `DISPLAY_MOVED` | `Event::DisplayMoved` | `event_handler_display_moved` |
| `DISPLAY_RESIZED` | `Event::DisplayResized` | `event_handler_display_resized` |
| `DISPLAY_CHANGED` | `Event::DisplayChanged` | `event_handler_display_changed` |
| `MOUSE_DOWN` | `Event::MouseDown` | `event_handler_mouse_down` |
| `MOUSE_UP` | `Event::MouseUp` | `event_handler_mouse_up` |
| `MOUSE_DRAGGED` | `Event::MouseDragged` | `event_handler_mouse_dragged` |
| `MOUSE_MOVED` | `Event::MouseMoved` | `event_handler_mouse_moved` |
| `MISSION_CONTROL_SHOW_ALL_WINDOWS` | `Event::MissionControlShowAllWindows` | `event_handler_mission_control_show_all_windows` |
| `MISSION_CONTROL_SHOW_FRONT_WINDOWS` | `Event::MissionControlShowFrontWindows` | `event_handler_mission_control_show_front_windows` |
| `MISSION_CONTROL_SHOW_DESKTOP` | `Event::MissionControlShowDesktop` | `event_handler_mission_control_show_desktop` |
| `MISSION_CONTROL_ENTER` | `Event::MissionControlEnter` | `event_handler_mission_control_enter` |
| `MISSION_CONTROL_CHECK_FOR_EXIT` | `Event::MissionControlCheckForExit` | `event_handler_mission_control_check_for_exit` |
| `MISSION_CONTROL_EXIT` | `Event::MissionControlExit` | `event_handler_mission_control_exit` |
| `DOCK_DID_RESTART` | `Event::DockDidRestart` | `event_handler_dock_did_restart` |
| `MENU_OPENED` | `Event::MenuOpened` | `event_handler_menu_opened` |
| `MENU_CLOSED` | `Event::MenuClosed` | `event_handler_menu_closed` |
| `MENU_BAR_HIDDEN_CHANGED` | `Event::MenuBarHiddenChanged` | `event_handler_menu_bar_hidden_changed` |
| `DOCK_DID_CHANGE_PREF` | `Event::DockDidChangePref` | `event_handler_dock_did_change_pref` |
| `SYSTEM_WOKE` | `Event::SystemWoke` | `event_handler_system_woke` |
| `DAEMON_MESSAGE` | `Event::DaemonMessage` | `event_handler_daemon_message` |

### 4.3 `SpaceOpError` — `space_manager.h:32-45`

| C | Rust | value |
| --- | --- | --- |
| `SPACE_OP_ERROR_SUCCESS` | `SpaceOpError::Success` | 0 |
| `SPACE_OP_ERROR_MISSING_SRC` | `SpaceOpError::MissingSrc` | 1 |
| `SPACE_OP_ERROR_MISSING_DST` | `SpaceOpError::MissingDst` | 2 |
| `SPACE_OP_ERROR_INVALID_SRC` | `SpaceOpError::InvalidSrc` | 3 |
| `SPACE_OP_ERROR_INVALID_DST` | `SpaceOpError::InvalidDst` | 4 |
| `SPACE_OP_ERROR_INVALID_TYPE` | `SpaceOpError::InvalidType` | 5 |
| `SPACE_OP_ERROR_SAME_SPACE` | `SpaceOpError::SameSpace` | 6 |
| `SPACE_OP_ERROR_SAME_DISPLAY` | `SpaceOpError::SameDisplay` | 7 |
| `SPACE_OP_ERROR_DISPLAY_IS_ANIMATING` | `SpaceOpError::DisplayIsAnimating` | 8 |
| `SPACE_OP_ERROR_IN_MISSION_CONTROL` | `SpaceOpError::InMissionControl` | 9 |
| `SPACE_OP_ERROR_SCRIPTING_ADDITION` | `SpaceOpError::ScriptingAddition` | 10 |

`Src` and `Dst` stay. Do not "fix" them to `Source` / `Destination` — that rule applies to
fields, parameters and locals, never to a variant.

### 4.4 `WindowOpError` — `window_manager.h:8-24`

| C | Rust |
| --- | --- |
| `WINDOW_OP_ERROR_SUCCESS` | `WindowOpError::Success` |
| `WINDOW_OP_ERROR_INVALID_SRC_VIEW` | `WindowOpError::InvalidSrcView` |
| `WINDOW_OP_ERROR_INVALID_SRC_NODE` | `WindowOpError::InvalidSrcNode` |
| `WINDOW_OP_ERROR_INVALID_DST_VIEW` | `WindowOpError::InvalidDstView` |
| `WINDOW_OP_ERROR_INVALID_DST_NODE` | `WindowOpError::InvalidDstNode` |
| `WINDOW_OP_ERROR_INVALID_OPERATION` | `WindowOpError::InvalidOperation` |
| `WINDOW_OP_ERROR_SAME_WINDOW` | `WindowOpError::SameWindow` |
| `WINDOW_OP_ERROR_CANT_MINIMIZE` | `WindowOpError::CantMinimize` |
| `WINDOW_OP_ERROR_ALREADY_MINIMIZED` | `WindowOpError::AlreadyMinimized` |
| `WINDOW_OP_ERROR_MINIMIZE_FAILED` | `WindowOpError::MinimizeFailed` |
| `WINDOW_OP_ERROR_NOT_MINIMIZED` | `WindowOpError::NotMinimized` |
| `WINDOW_OP_ERROR_DEMINIMIZE_FAILED` | `WindowOpError::DeminimizeFailed` |
| `WINDOW_OP_ERROR_MAX_STACK` | `WindowOpError::MaxStack` |
| `WINDOW_OP_ERROR_SAME_STACK` | `WindowOpError::SameStack` |

### 4.5 View and node enums — `view.h`

| C | Rust |
| --- | --- |
| `INSERT_FOCUSED` | `WindowInsertionPoint::Focused` (0) |
| `INSERT_FIRST` | `WindowInsertionPoint::First` (1) |
| `INSERT_LAST` | `WindowInsertionPoint::Last` (2) |
| `CHILD_NONE` | `WindowNodeChild::None` (0) |
| `CHILD_SECOND` | `WindowNodeChild::Second` (1) |
| `CHILD_FIRST` | `WindowNodeChild::First` (2) |
| `SPLIT_NONE` | `WindowNodeSplit::None` (0) |
| `SPLIT_Y` | `WindowNodeSplit::Y` (1) |
| `SPLIT_X` | `WindowNodeSplit::X` (2) |
| `SPLIT_AUTO` | `WindowNodeSplit::Auto` (3) |
| `VIEW_DEFAULT` | `ViewType::Default` (0) |
| `VIEW_BSP` | `ViewType::Bsp` (1) |
| `VIEW_STACK` | `ViewType::Stack` (2) |
| `VIEW_FLOAT` | `ViewType::Float` (3) |

### 4.6 Window-manager mode enums — `window_manager.h`

| C | Rust |
| --- | --- |
| `PURIFY_DISABLED` | `PurifyMode::Disabled` (0) |
| `PURIFY_MANAGED` | `PurifyMode::Managed` (1) |
| `PURIFY_ALWAYS` | `PurifyMode::Always` (2) |
| `FFM_DISABLED` | `FfmMode::Disabled` (0) |
| `FFM_AUTOFOCUS` | `FfmMode::Autofocus` (1) |
| `FFM_AUTORAISE` | `FfmMode::Autoraise` (2) |
| `WINDOW_ORIGIN_DEFAULT` | `WindowOriginMode::Default` (0) |
| `WINDOW_ORIGIN_FOCUSED` | `WindowOriginMode::Focused` (1) |
| `WINDOW_ORIGIN_CURSOR` | `WindowOriginMode::Cursor` (2) |

### 4.7 Display-manager enums — `display_manager.h`

| C | Rust |
| --- | --- |
| `DISPLAY_ARRANGEMENT_ORDER_DEFAULT` | `DisplayArrangementOrder::Default` (0) |
| `DISPLAY_ARRANGEMENT_ORDER_X` | `DisplayArrangementOrder::X` (1) |
| `DISPLAY_ARRANGEMENT_ORDER_Y` | `DisplayArrangementOrder::Y` (2) |
| `EXTERNAL_BAR_OFF` | `ExternalBarMode::Off` (0) |
| `EXTERNAL_BAR_MAIN` | `ExternalBarMode::Main` (1) |
| `EXTERNAL_BAR_ALL` | `ExternalBarMode::All` (2) |

### 4.8 Mouse enums — `mouse_handler.h`

| C | Rust |
| --- | --- |
| `MOUSE_DROP_ACTION_NONE` | `MouseDropAction::None` (0) |
| `MOUSE_DROP_ACTION_STACK` | `MouseDropAction::Stack` (1) |
| `MOUSE_DROP_ACTION_SWAP` | `MouseDropAction::Swap` (2) |
| `MOUSE_DROP_ACTION_WARP_TOP` | `MouseDropAction::WarpTop` (3) |
| `MOUSE_DROP_ACTION_WARP_RIGHT` | `MouseDropAction::WarpRight` (4) |
| `MOUSE_DROP_ACTION_WARP_BOTTOM` | `MouseDropAction::WarpBottom` (5) |
| `MOUSE_DROP_ACTION_WARP_LEFT` | `MouseDropAction::WarpLeft` (6) |
| `MOUSE_MODE_NONE` | `MouseMode::None` (0) |
| `MOUSE_MODE_MOVE` | `MouseMode::Move` (1) |
| `MOUSE_MODE_RESIZE` | `MouseMode::Resize` (2) |
| `MOUSE_MODE_SWAP` | `MouseMode::Swap` (3) |
| `MOUSE_MODE_STACK` | `MouseMode::Stack` (4) |

### 4.9 `MissionControlMode` — `mission_control.c:29-36`

| C | Rust | value |
| --- | --- | --- |
| `MISSION_CONTROL_MODE_INACTIVE` | `MissionControlMode::Inactive` | 0 |
| `MISSION_CONTROL_MODE_SHOW` | `MissionControlMode::Show` | 1 |
| `MISSION_CONTROL_MODE_SHOW_ALL_WINDOWS` | `MissionControlMode::ShowAllWindows` | 2 |
| `MISSION_CONTROL_MODE_SHOW_FRONT_WINDOWS` | `MissionControlMode::ShowFrontWindows` | 3 |
| `MISSION_CONTROL_MODE_SHOW_DESKTOP` | `MissionControlMode::ShowDesktop` | 4 |

### 4.10 `AnimationEasingType` — `misc/helpers.h:4-33`, `#[repr(usize)]`

| C | Rust | value | string |
| --- | --- | --- | --- |
| `ease_in_sine_type` | `AnimationEasingType::EaseInSine` | 0 | `"ease_in_sine"` |
| `ease_out_sine_type` | `AnimationEasingType::EaseOutSine` | 1 | `"ease_out_sine"` |
| `ease_in_out_sine_type` | `AnimationEasingType::EaseInOutSine` | 2 | `"ease_in_out_sine"` |
| `ease_in_quad_type` | `AnimationEasingType::EaseInQuad` | 3 | `"ease_in_quad"` |
| `ease_out_quad_type` | `AnimationEasingType::EaseOutQuad` | 4 | `"ease_out_quad"` |
| `ease_in_out_quad_type` | `AnimationEasingType::EaseInOutQuad` | 5 | `"ease_in_out_quad"` |
| `ease_in_cubic_type` | `AnimationEasingType::EaseInCubic` | 6 | `"ease_in_cubic"` |
| `ease_out_cubic_type` | `AnimationEasingType::EaseOutCubic` | 7 | `"ease_out_cubic"` |
| `ease_in_out_cubic_type` | `AnimationEasingType::EaseInOutCubic` | 8 | `"ease_in_out_cubic"` |
| `ease_in_quart_type` | `AnimationEasingType::EaseInQuart` | 9 | `"ease_in_quart"` |
| `ease_out_quart_type` | `AnimationEasingType::EaseOutQuart` | 10 | `"ease_out_quart"` |
| `ease_in_out_quart_type` | `AnimationEasingType::EaseInOutQuart` | 11 | `"ease_in_out_quart"` |
| `ease_in_quint_type` | `AnimationEasingType::EaseInQuint` | 12 | `"ease_in_quint"` |
| `ease_out_quint_type` | `AnimationEasingType::EaseOutQuint` | 13 | `"ease_out_quint"` |
| `ease_in_out_quint_type` | `AnimationEasingType::EaseInOutQuint` | 14 | `"ease_in_out_quint"` |
| `ease_in_expo_type` | `AnimationEasingType::EaseInExpo` | 15 | `"ease_in_expo"` |
| `ease_out_expo_type` | `AnimationEasingType::EaseOutExpo` | 16 | `"ease_out_expo"` |
| `ease_in_out_expo_type` | `AnimationEasingType::EaseInOutExpo` | 17 | `"ease_in_out_expo"` |
| `ease_in_circ_type` | `AnimationEasingType::EaseInCirc` | 18 | `"ease_in_circ"` |
| `ease_out_circ_type` | `AnimationEasingType::EaseOutCirc` | 19 | `"ease_out_circ"` |
| `ease_in_out_circ_type` | `AnimationEasingType::EaseInOutCirc` | 20 | `"ease_in_out_circ"` |
| `EASING_TYPE_COUNT` | `pub(crate) const EASING_TYPE_COUNT: usize = 21;` | not a variant | |

The 21 easing **functions** keep their C names unchanged: `ease_in_sine` … `ease_in_out_circ`,
each `fn(f32) -> f32` with the parameter named `interpolant`.

### 4.11 Message-parser enums — `message.c`

| C | Rust |
| --- | --- |
| `TOKEN_TYPE_INVALID` | `TokenType::Invalid` |
| `TOKEN_TYPE_UNKNOWN` | `TokenType::Unknown` — kept, never constructed |
| `TOKEN_TYPE_INT` | `TokenType::Int(i32)` |
| `TOKEN_TYPE_FLOAT` | `TokenType::Float(f32)` |
| `TOKEN_TYPE_U32` | `TokenType::U32(u32)` |
| `TOKEN_TYPE_STRING` | `TokenType::String` |
| `LABEL_DISPLAY` | `LabelType::Display` |
| `LABEL_SPACE` | `LabelType::Space` |
| `LABEL_WINDOW` | `LabelType::Window` |

### 4.12 Tri-state property constants

| C | Rust |
| --- | --- |
| `SIGNAL_PROP_UD` (`event_signal.h:90`) | `SignalProp::Undefined` = 0 |
| `SIGNAL_PROP_YES` (`:91`) | `SignalProp::Yes` = 1 |
| `SIGNAL_PROP_NO` (`:92`) | `SignalProp::No` = 2 |
| `RULE_PROP_UD` (`rule.h:4`) | `pub(crate) const RULE_PROP_UD: i32 = 0;` — stays an `i32` const, the fields are `int` |
| `RULE_PROP_ON` (`:5`) | `pub(crate) const RULE_PROP_ON: i32 = 1;` |
| `RULE_PROP_OFF` (`:6`) | `pub(crate) const RULE_PROP_OFF: i32 = 2;` |
| `REGEX_MATCH_UD` (`misc/macros.h:22`) | `RegexMatch::Undefined` = 0 |
| `REGEX_MATCH_YES` (`:23`) | `RegexMatch::Yes` = 1 |
| `REGEX_MATCH_NO` (`:24`) | `RegexMatch::No` = 2 |

### 4.13 `SaOpcode` — `osax/common.h:25-46`, generated by `build.rs`

| C | Rust | value |
| --- | --- | --- |
| `SA_OPCODE_HANDSHAKE` | `SaOpcode::Handshake` | 0x01 |
| `SA_OPCODE_SPACE_FOCUS` | `SaOpcode::SpaceFocus` | 0x02 |
| `SA_OPCODE_SPACE_CREATE` | `SaOpcode::SpaceCreate` | 0x03 |
| `SA_OPCODE_SPACE_DESTROY` | `SaOpcode::SpaceDestroy` | 0x04 |
| `SA_OPCODE_SPACE_MOVE` | `SaOpcode::SpaceMove` | 0x05 |
| `SA_OPCODE_WINDOW_MOVE` | `SaOpcode::WindowMove` | 0x06 |
| `SA_OPCODE_WINDOW_OPACITY` | `SaOpcode::WindowOpacity` | 0x07 |
| `SA_OPCODE_WINDOW_OPACITY_FADE` | `SaOpcode::WindowOpacityFade` | 0x08 |
| `SA_OPCODE_WINDOW_LAYER` | `SaOpcode::WindowLayer` | 0x09 |
| `SA_OPCODE_WINDOW_STICKY` | `SaOpcode::WindowSticky` | 0x0A |
| `SA_OPCODE_WINDOW_SHADOW` | `SaOpcode::WindowShadow` | 0x0B |
| `SA_OPCODE_WINDOW_FOCUS` | `SaOpcode::WindowFocus` | 0x0C |
| `SA_OPCODE_WINDOW_SCALE` | `SaOpcode::WindowScale` | 0x0D |
| `SA_OPCODE_WINDOW_SWAP_PROXY_IN` | `SaOpcode::WindowSwapProxyIn` | 0x0E |
| `SA_OPCODE_WINDOW_SWAP_PROXY_OUT` | `SaOpcode::WindowSwapProxyOut` | 0x0F |
| `SA_OPCODE_WINDOW_ORDER` | `SaOpcode::WindowOrder` | 0x10 |
| `SA_OPCODE_WINDOW_ORDER_IN` | `SaOpcode::WindowOrderIn` | 0x11 |
| `SA_OPCODE_WINDOW_LIST_TO_SPACE` | `SaOpcode::WindowListToSpace` | 0x12 |
| `SA_OPCODE_WINDOW_TO_SPACE` | `SaOpcode::WindowToSpace` | 0x13 |

---

## 5. Flag-set newtypes and their associated constants

Shape: `#[derive(Clone, Copy, PartialEq, Eq)] pub(crate) struct Name(pub uN);` with
`pub(crate) const` members. Constants drop the type prefix and keep the rest, uppercase.

### 5.1 `WindowFlag(pub u8)` — `window.h:108-118`

| C | Rust | value |
| --- | --- | --- |
| `WINDOW_SHADOW` | `WindowFlag::SHADOW` | 0x01 |
| `WINDOW_FULLSCREEN` | `WindowFlag::FULLSCREEN` | 0x02 |
| `WINDOW_MINIMIZE` | `WindowFlag::MINIMIZE` | 0x04 |
| `WINDOW_FLOAT` | `WindowFlag::FLOAT` | 0x08 |
| `WINDOW_STICKY` | `WindowFlag::STICKY` | 0x10 |
| `WINDOW_WINDOWED` | `WindowFlag::WINDOWED` | 0x20 |
| `WINDOW_MOVABLE` | `WindowFlag::MOVABLE` | 0x40 |
| `WINDOW_RESIZABLE` | `WindowFlag::RESIZABLE` | 0x80 |

### 5.2 `WindowRuleFlag(pub u8)` — `window.h:120-126`

| C | Rust | value |
| --- | --- | --- |
| `WINDOW_RULE_MANAGED` | `WindowRuleFlag::MANAGED` | 0x01 |
| `WINDOW_RULE_FULLSCREEN` | `WindowRuleFlag::FULLSCREEN` | 0x02 |
| `WINDOW_RULE_MFF` | `WindowRuleFlag::MFF` | 0x04 |
| `WINDOW_RULE_MFF_VALUE` | `WindowRuleFlag::MFF_VALUE` | 0x08 |

### 5.3 `RuleFlag(pub u16)` — `rule.h:8-20`

| C | Rust | value |
| --- | --- | --- |
| `RULE_APP_VALID` | `RuleFlag::APP_VALID` | 0x001 |
| `RULE_TITLE_VALID` | `RuleFlag::TITLE_VALID` | 0x002 |
| `RULE_ROLE_VALID` | `RuleFlag::ROLE_VALID` | 0x004 |
| `RULE_SUBROLE_VALID` | `RuleFlag::SUBROLE_VALID` | 0x008 |
| `RULE_APP_EXCLUDE` | `RuleFlag::APP_EXCLUDE` | 0x010 |
| `RULE_TITLE_EXCLUDE` | `RuleFlag::TITLE_EXCLUDE` | 0x020 |
| `RULE_ROLE_EXCLUDE` | `RuleFlag::ROLE_EXCLUDE` | 0x040 |
| `RULE_SUBROLE_EXCLUDE` | `RuleFlag::SUBROLE_EXCLUDE` | 0x080 |
| `RULE_ONE_SHOT` | `RuleFlag::ONE_SHOT` | 0x100 |
| `RULE_ONE_SHOT_REMOVE` | `RuleFlag::ONE_SHOT_REMOVE` | 0x200 |

The four `*_VALID` bits are still **published** by `rule_serialize`, derived from
`Option::is_some`, even though the field itself is an `Option`.

### 5.4 `RuleEffectsFlag(pub u16)` — `rule.h:22-27`

| C | Rust | value |
| --- | --- | --- |
| `RULE_FOLLOW_SPACE` | `RuleEffectsFlag::FOLLOW_SPACE` | 0x01 |
| `RULE_OPACITY` | `RuleEffectsFlag::OPACITY` | 0x02 |
| `RULE_LAYER` | `RuleEffectsFlag::LAYER` | 0x04 |

### 5.5 `ViewFlag(pub u64)` — `view.h:185-199`

| C | Rust | value |
| --- | --- | --- |
| `VIEW_LAYOUT` | `ViewFlag::LAYOUT` | 0x001 |
| `VIEW_TOP_PADDING` | `ViewFlag::TOP_PADDING` | 0x002 |
| `VIEW_BOTTOM_PADDING` | `ViewFlag::BOTTOM_PADDING` | 0x004 |
| `VIEW_LEFT_PADDING` | `ViewFlag::LEFT_PADDING` | 0x008 |
| `VIEW_RIGHT_PADDING` | `ViewFlag::RIGHT_PADDING` | 0x010 |
| `VIEW_WINDOW_GAP` | `ViewFlag::WINDOW_GAP` | 0x020 |
| `VIEW_AUTO_BALANCE` | `ViewFlag::AUTO_BALANCE` | 0x040 |
| `VIEW_ENABLE_PADDING` | `ViewFlag::ENABLE_PADDING` | 0x080 |
| `VIEW_ENABLE_GAP` | `ViewFlag::ENABLE_GAP` | 0x100 |
| `VIEW_IS_VALID` | `ViewFlag::IS_VALID` | 0x200 |
| `VIEW_IS_DIRTY` | `ViewFlag::IS_DIRTY` | 0x400 |
| `VIEW_SPLIT_TYPE` | `ViewFlag::SPLIT_TYPE` | 0x800 |

### 5.6 `MouseMod(pub u8)` — `mouse_handler.h:34-42`

| C | Rust | value |
| --- | --- | --- |
| `MOUSE_MOD_NONE` | `MouseMod::NONE` | 0x01 |
| `MOUSE_MOD_ALT` | `MouseMod::ALT` | 0x02 |
| `MOUSE_MOD_SHIFT` | `MouseMod::SHIFT` | 0x04 |
| `MOUSE_MOD_CMD` | `MouseMod::CMD` | 0x08 |
| `MOUSE_MOD_CTRL` | `MouseMod::CTRL` | 0x10 |
| `MOUSE_MOD_FN` | `MouseMod::FN` | 0x20 |

### 5.7 `AxWindowNotification(pub u8)` — `window.h:6-15`

| C | Rust | value |
| --- | --- | --- |
| `AX_WINDOW_MINIMIZED_INDEX` | `pub(crate) const AX_WINDOW_MINIMIZED_INDEX: usize = 0;` | 0 |
| `AX_WINDOW_DEMINIMIZED_INDEX` | `pub(crate) const AX_WINDOW_DEMINIMIZED_INDEX: usize = 1;` | 1 |
| `AX_WINDOW_DESTROYED_INDEX` | `pub(crate) const AX_WINDOW_DESTROYED_INDEX: usize = 2;` | 2 |
| `AX_WINDOW_MINIMIZED` | `AxWindowNotification::MINIMIZED` | 0x01 |
| `AX_WINDOW_DEMINIMIZED` | `AxWindowNotification::DEMINIMIZED` | 0x02 |
| `AX_WINDOW_DESTROYED` | `AxWindowNotification::DESTROYED` | 0x04 |
| `AX_WINDOW_ALL` | `AxWindowNotification::ALL` | 0x07, written as the OR of the three |

### 5.8 `AxApplicationNotification(pub u8)` — `application.h:7-24`

| C | Rust | value |
| --- | --- | --- |
| `AX_APPLICATION_WINDOW_CREATED_INDEX` | `AX_APPLICATION_WINDOW_CREATED_INDEX: usize` | 0 |
| `AX_APPLICATION_WINDOW_FOCUSED_INDEX` | `AX_APPLICATION_WINDOW_FOCUSED_INDEX: usize` | 1 |
| `AX_APPLICATION_WINDOW_MOVED_INDEX` | `AX_APPLICATION_WINDOW_MOVED_INDEX: usize` | 2 |
| `AX_APPLICATION_WINDOW_RESIZED_INDEX` | `AX_APPLICATION_WINDOW_RESIZED_INDEX: usize` | 3 |
| `AX_APPLICATION_WINDOW_TITLE_CHANGED_INDEX` | `AX_APPLICATION_WINDOW_TITLE_CHANGED_INDEX: usize` | 4 |
| `AX_APPLICATION_WINDOW_MENU_OPENED_INDEX` | `AX_APPLICATION_WINDOW_MENU_OPENED_INDEX: usize` | 5 |
| `AX_APPLICATION_WINDOW_MENU_CLOSED_INDEX` | `AX_APPLICATION_WINDOW_MENU_CLOSED_INDEX: usize` | 6 |
| `AX_APPLICATION_WINDOW_CREATED` | `AxApplicationNotification::WINDOW_CREATED` | 0x01 |
| `AX_APPLICATION_WINDOW_FOCUSED` | `AxApplicationNotification::WINDOW_FOCUSED` | 0x02 |
| `AX_APPLICATION_WINDOW_MOVED` | `AxApplicationNotification::WINDOW_MOVED` | 0x04 |
| `AX_APPLICATION_WINDOW_RESIZED` | `AxApplicationNotification::WINDOW_RESIZED` | 0x08 |
| `AX_APPLICATION_WINDOW_TITLE_CHANGED` | `AxApplicationNotification::WINDOW_TITLE_CHANGED` | 0x10 |
| `AX_APPLICATION_ALL` | `AxApplicationNotification::ALL` | **0x1F** — the first five only; never derived from the table length |

### 5.9 `ResizeHandle(pub u8)` — `misc/macros.h:36-40`

| C | Rust | value |
| --- | --- | --- |
| `HANDLE_TOP` | `ResizeHandle::TOP` | 0x01 |
| `HANDLE_BOTTOM` | `ResizeHandle::BOTTOM` | 0x02 |
| `HANDLE_LEFT` | `ResizeHandle::LEFT` | 0x04 |
| `HANDLE_RIGHT` | `ResizeHandle::RIGHT` | 0x08 |
| `HANDLE_ABS` | `ResizeHandle::ABS` | 0x10 |

### 5.10 `OsaxAttrib(pub u32)` — `osax/common.h:9-23`, generated by `build.rs`

| C | Rust | value |
| --- | --- | --- |
| `OSAX_ATTRIB_DOCK_SPACES` | `OsaxAttrib::DOCK_SPACES` | 0x01 |
| `OSAX_ATTRIB_DPPM` | `OsaxAttrib::DPPM` | 0x02 |
| `OSAX_ATTRIB_ADD_SPACE` | `OsaxAttrib::ADD_SPACE` | 0x04 |
| `OSAX_ATTRIB_REM_SPACE` | `OsaxAttrib::REM_SPACE` | 0x08 |
| `OSAX_ATTRIB_MOV_SPACE` | `OsaxAttrib::MOV_SPACE` | 0x10 |
| `OSAX_ATTRIB_SET_WINDOW` | `OsaxAttrib::SET_WINDOW` | 0x20 |
| `OSAX_ATTRIB_ANIM_TIME` | `OsaxAttrib::ANIM_TIME` | 0x40 |
| `OSAX_ATTRIB_ALL` | `OsaxAttrib::ALL` | 0x7F |

`DPPM`, `REM`, `MOV` and `ANIM` are the wire protocol's own words. Do not expand them.

---

## 6. Property masks — `u64` constants, not enums, not newtypes

### 6.1 `WINDOW_PROPERTY_*` — `window.h:31-64`, 33 constants, names unchanged

`ID 0x000000001`, `PID 0x000000002`, `APP 0x000000004`, `TITLE 0x000000008`,
`SCRATCHPAD 0x000000010`, `FRAME 0x000000020`, `ROLE 0x000000040`, `SUBROLE 0x000000080`,
`ROOT_WINDOW 0x000000100`, `DISPLAY 0x000000200`, `SPACE 0x000000400`, `LEVEL 0x000000800`,
`SUB_LEVEL 0x000001000`, `LAYER 0x000002000`, `SUB_LAYER 0x000004000`, `OPACITY 0x000008000`,
`SPLIT_TYPE 0x000010000`, `SPLIT_CHILD 0x000020000`, `STACK_INDEX 0x000040000`,
`CAN_MOVE 0x000080000`, `CAN_RESIZE 0x000100000`, `HAS_FOCUS 0x000200000`,
`HAS_SHADOW 0x000400000`, `HAS_PARENT_ZOOM 0x000800000`, `HAS_FULLSCREEN_ZOOM 0x001000000`,
`HAS_AX_REFERENCE 0x002000000`, `IS_FULLSCREEN 0x004000000`, `IS_VISIBLE 0x008000000`,
`IS_MINIMIZED 0x010000000`, `IS_HIDDEN 0x020000000`, `IS_FLOATING 0x040000000`,
`IS_STICKY 0x080000000`, `IS_GRABBED 0x100000000` — each `pub(crate) const WINDOW_PROPERTY_X: u64`.

### 6.2 `SPACE_PROPERTY_*` — `view.h:7-19`, 12 constants, names unchanged

`ID 0x001`, `UUID 0x002`, `INDEX 0x004`, `LABEL 0x008`, `TYPE 0x010`, `DISPLAY 0x020`,
`WINDOWS 0x040`, `FIRST_WINDOW 0x080`, `LAST_WINDOW 0x100`, `HAS_FOCUS 0x200`,
`IS_VISIBLE 0x400`, `IS_FULLSCREEN 0x800`.

### 6.3 `DISPLAY_PROPERTY_*` — `display.h:7-14`, 7 constants, names unchanged

`ID 0x01`, `UUID 0x02`, `INDEX 0x04`, `LABEL 0x08`, `FRAME 0x10`, `SPACES 0x20`, `HAS_FOCUS 0x40`.

---

## 7. Constants

### 7.1 `src/misc/macros.h`

| C | Rust |
| --- | --- |
| `KILOBYTES(value)` | `pub(crate) const fn kilobytes(value: u64) -> u64` |
| `MEGABYTES(value)` | `pub(crate) const fn megabytes(value: u64) -> u64` |
| `GIGABYTES(value)` | **not translated** — no call site |
| `array_count(a)` | no item; write `array.len() as i32` |
| `min(a, b)` | **not translated** — no call site |
| `max(a, b)` | `pub(crate) fn max<T: PartialOrd>(first: T, second: T) -> T` |
| `add_and_clamp_to_zero(a, b)` | `pub(crate) fn add_and_clamp_to_zero(value: i32, delta: i32) -> i32` |
| `in_range_ii(a, b, c)` | `pub(crate) fn in_range_ii<T: PartialOrd>(value: T, low: T, high: T) -> bool` |
| `in_range_ie(a, b, c)` | `in_range_ie`, same signature and parameter names |
| `in_range_ei(a, b, c)` | `in_range_ei`, same signature and parameter names — **a distinct function**, never a flag on `in_range_ii` |
| `in_range_ee(a, b, c)` | **not translated** — no call site |
| `lerp(a, t, b)` | `pub(crate) fn lerp(start: f64, interpolant: f32, end: f32) -> f64` |
| `FAILURE_MESSAGE` | `pub(crate) const FAILURE_MESSAGE: &[u8] = b"\x07";` |
| `MAXLEN` | `pub(crate) const MAXLEN: usize = 512;` |
| `DIR_NORTH` | `pub(crate) const DIR_NORTH: i32 = 360;` |
| `DIR_EAST` | `pub(crate) const DIR_EAST: i32 = 90;` |
| `DIR_SOUTH` | `pub(crate) const DIR_SOUTH: i32 = 180;` |
| `DIR_WEST` | `pub(crate) const DIR_WEST: i32 = 270;` |
| `STACK` | `pub(crate) const STACK: i32 = 111;` |
| `TYPE_ABS` | `pub(crate) const TYPE_ABS: i32 = 0x1;` |
| `TYPE_REL` | `pub(crate) const TYPE_REL: i32 = 0x2;` |
| `LAYER_AUTO` | `pub(crate) const LAYER_AUTO: i32 = 0;` |
| `LAYER_BELOW` | `pub(crate) const LAYER_BELOW: i32 = 3;` |
| `LAYER_NORMAL` | `pub(crate) const LAYER_NORMAL: i32 = 4;` |
| `LAYER_ABOVE` | `pub(crate) const LAYER_ABOVE: i32 = 5;` |

### 7.2 `src/view.h`, `src/view.c`, `src/window_manager.h`, `src/display_manager.h`

| C | Rust |
| --- | --- |
| `AX_ABS(a, b)` (`view.h:4`) | folded into `ax_diff` |
| `AX_DIFF(a, b)` (`view.h:5`) | `pub(crate) fn ax_diff(first: f64, second: f64) -> bool` |
| `NODE_MAX_WINDOW_COUNT` (`view.h:151`) | `pub(crate) const NODE_MAX_WINDOW_COUNT: usize = 32;` |
| `INSERT_FEEDBACK_WIDTH` (`view.c:6`) | `pub(crate) const INSERT_FEEDBACK_WIDTH: f64 = 2.0;` |
| `INSERT_FEEDBACK_RADIUS` (`view.c:7`) | `pub(crate) const INSERT_FEEDBACK_RADIUS: f64 = 9.0;` |
| `kCPSAllWindows` (`window_manager.h:4`) | `pub(crate) const kCPSAllWindows: u32 = 0x100;` with `#[allow(non_upper_case_globals)]` |
| `kCPSUserGenerated` (`:5`) | `pub(crate) const kCPSUserGenerated: u32 = 0x200;` |
| `kCPSNoWindows` (`:6`) | `pub(crate) const kCPSNoWindows: u32 = 0x400;` |
| `DOCK_ORIENTATION_BOTTOM` (`display_manager.h:4`) | `pub(crate) const DOCK_ORIENTATION_BOTTOM: i32 = 2;` |
| `DOCK_ORIENTATION_LEFT` (`:5`) | `pub(crate) const DOCK_ORIENTATION_LEFT: i32 = 3;` |
| `DOCK_ORIENTATION_RIGHT` (`:6`) | `pub(crate) const DOCK_ORIENTATION_RIGHT: i32 = 4;` |
| `MOUSE_EVENT_MASK_FFM` (`mouse_handler.h:4`) | `pub(crate) const MOUSE_EVENT_MASK_FFM: u32` — expression kept, including the bare `30` for `kCGSEventDockControl` |
| `MOUSE_EVENT_MASK` (`:13`) | `pub(crate) const MOUSE_EVENT_MASK: u32` |

### 7.3 `src/yabai.c`

| C | Rust |
| --- | --- |
| `SA_SOCKET_PATH_FMT` (`:1`) | `pub(crate) const SA_SOCKET_PATH_FMT: &str = "/tmp/yabai-sa_%s.socket";` |
| `SOCKET_PATH_FMT` (`:2`) | `pub(crate) const SOCKET_PATH_FMT: &str = "/tmp/yabai_%s.socket";` |
| `LCFILE_PATH_FMT` (`:3`) | `pub(crate) const LCFILE_PATH_FMT: &str = "/tmp/yabai_%s.lock";` |
| `SCRPT_ADD_LOAD_OPT` (`:5`) | `SCRPT_ADD_LOAD_OPT` — name unchanged |
| `SCRPT_ADD_UNINSTALL_OPT` (`:6`) | unchanged |
| `SERVICE_INSTALL_OPT` (`:7`) | unchanged |
| `SERVICE_UNINSTALL_OPT` (`:8`) | unchanged |
| `SERVICE_START_OPT` (`:9`) | unchanged |
| `SERVICE_RESTART_OPT` (`:10`) | unchanged |
| `SERVICE_STOP_OPT` (`:11`) | unchanged |
| `CLIENT_OPT_LONG` (`:12`) / `CLIENT_OPT_SHRT` (`:13`) | unchanged — `SHRT` is the C's own word |
| `CONFIG_OPT_LONG` (`:14`) / `CONFIG_OPT_SHRT` (`:15`) | unchanged |
| `DEBUG_VERBOSE_OPT_LONG` (`:16`) / `DEBUG_VERBOSE_OPT_SHRT` (`:17`) | unchanged |
| `VERSION_OPT_LONG` (`:18`) / `VERSION_OPT_SHRT` (`:19`) | unchanged |
| `HELP_OPT_LONG` (`:20`) / `HELP_OPT_SHRT` (`:21`) | unchanged |
| `MAJOR` (`:23`) | `pub(crate) const MAJOR: i32 = 7;` |
| `MINOR` (`:24`) | `pub(crate) const MINOR: i32 = 1;` |
| `PATCH` (`:25`) | `pub(crate) const PATCH: i32 = 25;` |

### 7.4 `src/message.c` — 205 string constants, lines 14-251

All are `#define NAME "text"` and all keep their C name exactly, as
`pub(crate) const NAME: &str = "text";`. The families:

| family | count | example |
| --- | --- | --- |
| `DOMAIN_*` | 7 | `DOMAIN_CONFIG = "config"` |
| `COMMAND_*` | 81 | `COMMAND_WINDOW_GRID` |
| `ARGUMENT_*` | 116 | `ARGUMENT_COMMON_SEL_NORTH` |
| `SELECTOR_*` | 1 | `SELECTOR_CONFIG_SPACE = "--space"` |

No translator renames, regroups or re-cases any of them; the string values are the CLI surface.

### 7.5 `src/sa.m`, `src/osax/common.h`, `src/misc/service.h`

| C | Rust |
| --- | --- |
| `CSR_ALLOW_UNRESTRICTED_FS` (`sa.m:4`) | `pub(crate) const CSR_ALLOW_UNRESTRICTED_FS: u32 = 0x02;` |
| `CSR_ALLOW_TASK_FOR_PID` (`sa.m:5`) | `pub(crate) const CSR_ALLOW_TASK_FOR_PID: u32 = 0x04;` |
| `sa_plist` (`sa.m:21`) | `pub(crate) const SA_PLIST: &str` |
| `sa_bundle_plist` (`sa.m:50`) | `pub(crate) const SA_BUNDLE_PLIST: &str` |
| `sa_payload_init()` (`sa.m:418`) | no item — a `[u8; SA_SOCKET_BUFF_LEN]` local plus `let mut length: i16 = 1 + 2;` |
| `pack(v)` (`sa.m:419`) | `fn pack(bytes: &mut [u8], length: &mut i16, value: &[u8]) -> bool` with the bounds check of `DECISIONS.md` 34 |
| `sa_payload_send(op)` (`sa.m:420`) | `fn sa_payload_send(bytes: &mut [u8], length: i16, opcode: SaOpcode) -> bool` |
| `SA_SOCKET_PATH_FMT` (`osax/common.h:4`) | generated const; identical text to `yabai.c:1` |
| `SA_SOCKET_BUFF_LEN` (`:5`) | `pub(crate) const SA_SOCKET_BUFF_LEN: usize = 0x1000;` |
| `OSAX_VERSION` (`:7`) | `pub(crate) const OSAX_VERSION: &str = "2.1.30";` |
| `_PATH_LAUNCHCTL` (`misc/service.h:4`) | `pub(crate) const _PATH_LAUNCHCTL: &str = "/bin/launchctl";` |
| `_NAME_YABAI_PLIST` (`:5`) | `pub(crate) const _NAME_YABAI_PLIST: &str = "com.asmvik.yabai";` |
| `_PATH_YABAI_PLIST` (`:6`) | `pub(crate) const _PATH_YABAI_PLIST: &str` — the concatenation is done once, in the const |
| `_YABAI_PLIST` (`:8`) | `pub(crate) const _YABAI_PLIST: &str` — byte-for-byte |

The leading underscores stay: they are the C names.

### 7.6 Liveness constants (`crate::window`)

| Rust | value |
| --- | --- |
| `WINDOW_LIVENESS_ALIVE` | `0u8` |
| `WINDOW_LIVENESS_CLAIMED_FOR_DESTRUCTION` | `1u8` |

---

## 8. Statics

### 8.1 Process-wide statics from `src/yabai.c:27-52`

| C global | Rust static or field |
| --- | --- |
| `g_signal_event` | field `EventLoopOwnedState::signal_event` |
| `g_process_manager` | field `process_manager` + `PROCESS_TABLE` + `CARBON_PROCESS_EVENT_INSTALLATION` |
| `g_display_manager` | field `display_manager` |
| `g_window_manager` | field `window_manager` |
| `g_space_manager` | field `space_manager` |
| `g_signal_storage` | field `signal_storage` |
| `g_mouse_state` | field `mouse_drag_state` + `static MOUSE_TAP_STATE: MouseTapState` |
| `g_event_loop` | `static EVENT_SENDER: OnceLock<Sender<Event>>`; the `Receiver<Event>` is a local of `event_loop_run` |
| `g_workspace_context` | `static WORKSPACE_CONTEXT: OnceLock<Retained<WorkspaceContext>>` |
| `g_mission_control_mode` | field `mission_control_mode` |
| `g_cv_host_clock_frequency` | `static CV_HOST_CLOCK_FREQUENCY: OnceLock<f64>` |
| `g_layer_normal_window_level` | `static LAYER_NORMAL_WINDOW_LEVEL: OnceLock<i32>` |
| `g_layer_below_window_level` | `static LAYER_BELOW_WINDOW_LEVEL: OnceLock<i32>` |
| `g_layer_above_window_level` | `static LAYER_ABOVE_WINDOW_LEVEL: OnceLock<i32>` |
| `g_event_bytes` | **gone** — a zeroed `[u8; 0x100]` local in each of the two users |
| `g_sa_socket_file` | `static SA_SOCKET_FILE: OnceLock<String>` |
| `g_socket_file` | `static SOCKET_FILE: OnceLock<String>` |
| `g_config_file` | `static CONFIG_FILE: OnceLock<String>` |
| `g_lock_file` | `static LOCK_FILE: OnceLock<String>` |
| `g_bs_port` | `static BOOTSTRAP_PORT: OnceLock<mach_port_t>` |
| `g_connection` | `static CONNECTION: OnceLock<i32>` |
| `g_verbose` | `static VERBOSE: AtomicBool` |
| `g_pid` | `static PROCESS_ID: OnceLock<i32>` |

### 8.2 File-scope statics elsewhere

| C static | site | Rust |
| --- | --- | --- |
| `__pending_window_focus` | `event_loop.c:11` | `static PENDING_WINDOW_FOCUS: AtomicBool` |
| `__pending_gesture` | `event_loop.c:12` | `static PENDING_GESTURE: AtomicBool` |
| `__last_gesture_time` | `event_loop.c:13` | `static LAST_GESTURE_TIME: AtomicU64` |
| `__last_cmd_tab_time` | `event_loop.c:14` | `static LAST_CMD_TAB_TIME: AtomicU64` |
| `ffm_value` | `event_loop.c:1561` | field `focus_follows_mouse_suspended_value` |
| `is_menu_open` | `event_loop.c:1562` | field `is_menu_open` |
| `g_message_loop` | `message.c:1` | `static MESSAGE_LOOP: OnceLock<MessageLoop>` |
| `g_mission_control_observer` | `mission_control.c:46` | `static MISSION_CONTROL_OBSERVER: Mutex<Option<MissionControlObserver>>` |
| `g_notify_init` | `misc/notify.h:4` | `static NOTIFY_INIT: AtomicBool` |
| `g_notify_img` | `misc/notify.h:5` | `static NOTIFY_IMAGE: OnceLock<Retained<NSImage>>` |
| `g_temp_storage` | `misc/ts.h:4` | **gone** |
| `g_profiler` | `misc/timer.h:16` | **gone** — dead code |
| `cpu_freq` | `misc/timer.h:38` | **gone** — dead code |
| `g_nsobject_autorelease`, `g_nsautoreleasepool_drain`, `g_nsautoreleasepool_release` | `misc/autorelease.h:3-5` | **gone** — dead code |
| `CGSGetConnectionPortById` | `misc/extern.h:4` | `static CGS_GET_CONNECTION_PORT_BY_ID: OnceLock<Option<extern "C" fn(i32) -> mach_port_t>>` |
| `SLSPerformAsynchronousBridgedWindowManagementOperation` | `misc/extern.h:5` | `static SLS_PERFORM_ASYNCHRONOUS_BRIDGED_WINDOW_MANAGEMENT_OPERATION: OnceLock<Option<extern "C" fn(*mut c_void) -> i64>>` |
| `osax_*` (11 buffers) | `sa.m:9-19` | `static OSAX_PATHS: OnceLock<OsaxPaths>` — §3.29 |
| `_workspace_is_macos_version_<name>` ×6 | `workspace.h:13` | `static _workspace_is_macos_version_<name>: AtomicBool` ×6, read by `workspace_is_macos_<name>()` |
| `process_name_blacklist` | `process_manager.c:14` | `const PROCESS_NAME_BLACKLIST: [&str; 4]` |
| `hash_wm` | `window_manager.c:9` | `fn hash_window_manager_key(key: &u32) -> u64` |
| `compare_wm` | `window_manager.c:14` | **gone** — `K: PartialEq` |
| `hash_view` | `space_manager.c:4` | `fn hash_view_key(key: &u64) -> u64` |
| `compare_view` | `space_manager.c:9` | **gone** |
| `hash_psn` | `process_manager.c:4` | `fn hash_process_serial_number(key: &ProcessSerialNumber) -> u64` |
| `compare_psn` | `process_manager.c:9` | **gone** — hand-written `PartialEq` comparing both longs |
| `static char process_name[…]` | `window.c:173` | a stack `[u8; PROC_PIDPATHINFO_MAXSIZE]` local |
| `static char process_name[…]` | `window_manager.c:935` | a **second, distinct** stack local |

### 8.3 `CFStringRef` constants

| C | Rust |
| --- | --- |
| `kAXFullscreenAttribute` (`window.h:4`) | `static K_AX_FULLSCREEN_ATTRIBUTE: OnceLock<CFStringOwned>` |
| `kAXEnhancedUserInterface` (`misc/helpers.h:171`) | `static K_AX_ENHANCED_USER_INTERFACE: OnceLock<CFStringOwned>` |
| `kAXExposeShowAllWindows` (`mission_control.c:52`) | `static K_AX_EXPOSE_SHOW_ALL_WINDOWS: OnceLock<CFStringOwned>` |
| `kAXExposeShowFrontWindows` (`:53`) | `static K_AX_EXPOSE_SHOW_FRONT_WINDOWS: OnceLock<CFStringOwned>` |
| `kAXExposeShowDesktop` (`:54`) | `static K_AX_EXPOSE_SHOW_DESKTOP: OnceLock<CFStringOwned>` |
| `kAXExposeExit` (`:55`) | `static K_AX_EXPOSE_EXIT: OnceLock<CFStringOwned>` |

---

## 9. String and lookup tables

`static` file-scope arrays become `SCREAMING_SNAKE`, same order, same holes.

| C | Rust | shape |
| --- | --- | --- |
| `ax_error_str` (`application.h:26`) | `AX_ERROR_STR` | `[&str; 16]`, indexed `(-result) as usize` |
| `ax_application_notification_str` (`application.h:46`) | `AX_APPLICATION_NOTIFICATION_STR` | `[&str; 7]` |
| `ax_application_notification` (`application.h:57`) | `AX_APPLICATION_NOTIFICATION` | `OnceLock<[CFStringOwned; 7]>` |
| `ax_window_notification_str` (`window.h:17`) | `AX_WINDOW_NOTIFICATION_STR` | `[&str; 3]` |
| `ax_window_notification` (`window.h:24`) | `AX_WINDOW_NOTIFICATION` | `OnceLock<[CFStringOwned; 3]>` |
| `display_property_val` (`display.h:23`) | `DISPLAY_PROPERTY_VAL` | `[u64; 7]` |
| `display_property_str` (`display.h:30`) | `DISPLAY_PROPERTY_STR` | `[&str; 7]` |
| `space_property_val` (`view.h:28`) | `SPACE_PROPERTY_VAL` | `[u64; 12]` |
| `space_property_str` (`view.h:35`) | `SPACE_PROPERTY_STR` | `[&str; 12]` |
| `window_property_val` (`window.h:73`) | `WINDOW_PROPERTY_VAL` | `[u64; 33]` |
| `window_property_str` (`window.h:80`) | `WINDOW_PROPERTY_STR` | `[&str; 33]` |
| `window_insertion_point_str` (`view.h:101`) | `WINDOW_INSERTION_POINT_STR` | `[&str; 3]` |
| `window_node_child_str` (`view.h:115`) | `WINDOW_NODE_CHILD_STR` | `[&str; 3]` |
| `window_node_split_str` (`view.h:130`) | `WINDOW_NODE_SPLIT_STR` | `[&str; 4]` |
| `auto_balance_str` (`view.h:138`) | `AUTO_BALANCE_STR` | `[&str; 4]` — a **second** table over the same values; keep both |
| `view_type_str` (`view.h:177`) | `VIEW_TYPE_STR` | `[&str; 4]` |
| `purify_mode_str` (`window_manager.h:33`) | `PURIFY_MODE_STR` | `[&str; 3]` — `["on", "float", "off"]`, inverted relative to the enum names; do not correct |
| `ffm_mode_str` (`window_manager.h:47`) | `FFM_MODE_STR` | `[&str; 3]` |
| `window_origin_mode_str` (`window_manager.h:61`) | `WINDOW_ORIGIN_MODE_STR` | `[&str; 3]` |
| `display_arrangement_order_str` (`display_manager.h:15`) | `DISPLAY_ARRANGEMENT_ORDER_STR` | `[&str; 3]` |
| `external_bar_mode_str` (`display_manager.h:29`) | `EXTERNAL_BAR_MODE_STR` | `[&str; 3]` |
| `signal_type_str` (`event_signal.h:47`) | `SIGNAL_TYPE_STR` | `[&str; 31]` — the 31st entry `"signal_type_count"` is present |
| `mouse_mod_str` (`mouse_handler.h:83`) | `MOUSE_MOD_STR` | `[Option<&str>; 33]`, sparse, indexed by the **bit value** |
| `mouse_mode_str` (`mouse_handler.h:93`) | `MOUSE_MODE_STR` | `[&str; 5]` |
| `mission_control_mode_str` (`mission_control.c:38`) | `MISSION_CONTROL_MODE_STR` | `[Option<&str>; 5]` |
| `animation_easing_type_str` (`misc/helpers.h:35`) | `ANIMATION_EASING_TYPE_STR` | `[&str; 21]` — indexed *and* iterated |
| `bool_str` (`misc/helpers.h:173`) | `BOOL_STR` | `[&str; 2]` = `["off", "on"]` |
| `layer_str` (`misc/helpers.h:175`) | `LAYER_STR` | `[Option<&str>; 6]`, `None` at 1 and 2 |
| `token_char_int_table` (`message.c:283`) | `TOKEN_CHAR_INT_TABLE` | `[i32; 103]`, sparse; the largest designated index is `'f'` = 102, and both readers (`message.c:352`, `:377`) guard the character first |
| `reserved_display_identifiers` (`message.c:515`) | `RESERVED_DISPLAY_IDENTIFIERS` | `[&str; 10]` |
| `reserved_space_identifiers` (`message.c:529`) | `RESERVED_SPACE_IDENTIFIERS` | `[&str; 6]` |
| `reserved_window_identifiers` (`message.c:539`) | `RESERVED_WINDOW_IDENTIFIERS` | `[&str; 11]` |

---

## 10. Parameter, local and loop-binding abbreviations

Left column is the C spelling; right column is the only Rust spelling. Applies to fields,
parameters, locals, loop bindings and closure captures. Never to function names, type names,
enum variants or `#define` string constants.

### 10.1 Managers and long-lived values

| C | Rust |
| --- | --- |
| `wm` | `window_manager` |
| `sm` | `space_manager` |
| `dm` | `display_manager` |
| `pm` | `process_manager` |
| `ms` | `mouse_state` (the drag half: `mouse_drag_state`) |
| `es` | `event_signal` (the record is a `PendingSignal`) |
| `rsp` | `response` — the `Response` of `DECISIONS.md` 28 |
| `pool` (`NSAutoreleasePool`) | `pool` |
| `link` (`CVDisplayLinkRef`) | `link` |

### 10.2 Identifiers

| C | Rust | Rust type |
| --- | --- | --- |
| `wid`, `window_id` | `window_id` | `WindowId` |
| `sid` | `space_id` | `SpaceId` |
| `did` | `display_id` | `DisplayId` |
| `pid` | `process_id` | `ProcessId` |
| `psn` | `process_serial_number` | `ProcessSerialNumber` |
| `cid` | `connection_id` | `i32` |
| `wcid` (`misc/extern.h:77`, `:83`) | `window_connection_id` | `i32` |
| `psn_cid` (`misc/extern.h:80`) | `process_serial_number_connection_id` | `i32` |
| `uuid` | `uuid` | unchanged |
| `mci` (`space_manager.c:824`) | `mission_control_index` | `i32` |
| `a_sid` / `b_sid` | `a_space_id` / `b_space_id` | |
| `n_sid` (`space_manager.c:560`) | `next_space_id` | |
| `p_sid` (`space_manager.c:559`) | `previous_space_id` | |
| `c_sid` (`space_manager.c:733`) | `current_space_id` | |
| `s_did` (`space_manager.c:897`) | `source_display_id` | |
| `d_sid` (`space_manager.c:909`) | `destination_space_id` | |
| `a_did` / `b_did` (`display_manager.c:144`, `space_manager.c:745`) | `a_display_id` / `b_display_id` | |
| `a_wid` / `b_wid` (`sa.m:576`) | `a_window_id` / `b_window_id` | |
| `rel_wid` (`misc/extern.h:33`) | `relative_window_id` | |
| `acting_did` / `acting_sid` | `acting_display_id` / `acting_space_id` | |
| `selector_sid` | `selector_space_id` | |
| `new_did` / `new_sid` | `new_display_id` / `new_space_id` | |
| `src_sid` / `dst_sid` / `src_prev_sid` (`sa.h:15`) | `source_space_id` / `destination_space_id` / `source_previous_space_id` | |
| `filter_wid` (`window_manager.h:125`) | `filter_window_id` | |
| `psn_sid` (`event_loop.c:359`) | `psn_space_id` | `psn` is part of the concept here |
| `front_pid` / `last_front_pid` | `front_process_id` / `last_front_process_id` | |
| `finder_psn` | `finder_process_serial_number` | |
| `window_psn` (`window_manager.h:149`) | `window_process_serial_number` | |
| `desktop_id` (`space_manager.h:63`) | `desktop_id` | unchanged |

### 10.3 Prefixes and suffixes

| C | Rust |
| --- | --- |
| `src_` (`src_view`, `src_node`, `src_window`, `src_node_add`, `src_node_rm`) | `source_` (`source_view`, `source_node`, `source_window`, `source_node_add`, `source_node_remove`) |
| `dst_` (`dst_view`, `dst_node`, `dst_window`) | `destination_` (`destination_view`, `destination_node`, `destination_window`) |
| `a_` / `b_` (`a_view`, `b_node`) | unchanged — `a_view`, `b_node` |
| bare `a` / `b` for two windows (`window_manager.c:1799`, `:1832`, `:1950`) | `a_window` / `b_window` |
| `a` / `b` in `string_equals` (`misc/helpers.h:254`) | `first` / `second` |
| `a` / `b` in `psn_equals` (`misc/helpers.h:534`) | `first` / `second` |
| `a` / `b` in `display_manager_coordinate_comparator` (`display_manager.c:140`) | `a_display` / `b_display` |
| `dview` (`window_manager.c:2142`) | `display_view` |
| `ws_context` (`workspace.m:8-12`) | `workspace_context` |
| `srole` (`rule.c:11`); `escaped_srole` | `subrole`; `escaped_subrole` |
| `app` as a field or local | `application` — **except** `Rule::app`, `Signal::app`, `PendingSignal::app` and the `ARGUMENT_*`/`WINDOW_PROPERTY_APP` strings, where `app` is the user-visible config word |
| `insert_dir` (`view.h:165`) | `insert_direction` |
| `dir` as a parameter (`view.c:509`, `:514`) | `direction` |
| `_ref` naming a CF value (`space_list_ref`, `sid_ref`, `uuid_ref`, `window_ref`, `observer_ref`, `app_ref`) | unchanged |

### 10.4 Scalars, buffers and loop bindings

| C | Rust |
| --- | --- |
| `ref` (`struct window::ref`, `struct application::ref`) | `element_ref` — mandatory, `ref` is a Rust keyword |
| `mod` (`event_loop.c:1134`, `mouse_handler.c:33`, `:64`) | `event_modifier` — mandatory; it is the modifier the event carried, never the configured one |
| `mod` in `ts_align` (`misc/ts.h:42`) | `misalignment` |
| `x_mod` / `y_mod` (`window_manager.c:350-351`) | `x_modifier` / `y_modifier` — the `-1`/`0`/`1` read off the `HANDLE_*` bits |
| `id_ptr` (`window.h:92`) | `id_pointer` if a local ever needs the C word; the **field** is `liveness` |
| `len` / `cap` (`misc/sbuffer.h:6-7`) | `length` / `capacity` |
| `buf` | `buffer` |
| `str` (`event_signal.c:343`) | `string` |
| `s` (`misc/helpers.h:259`, `:387`, `:397`) | `string` |
| `tmp` (`space_manager.c:769`) | `temporary` |
| `temp` | `temporary` |
| `prev` (`window_manager.c:1007`) | `previous` |
| `num` (`misc/helpers.h:321`, `:328`) | `number` |
| `ptr` (`misc/ts.h:40`) | `pointer` |
| `dst` / `cursor` in `ts_string_escape` (`misc/helpers.h:286`) | `destination` / `cursor` |
| `dt` (`event_loop.c:363`, `:1266`, `:1353`, `event_signal.c:156`) | `delta_time` |
| `dx` / `dy` | `delta_x` / `delta_y` |
| `dw` / `dh` (`mouse_handler.h:55`) | `delta_width` / `delta_height` |
| `changed_w` / `changed_h` (`mouse_handler.h:57`) | `changed_width` / `changed_height` |
| `tx` / `ty` / `tw` / `th` (`view.h:61`) | `target_x` / `target_y` / `target_width` / `target_height` |
| `fx` / `fy` / `fw` / `fh` (`window_manager.c:353-356`, `:2172-2175`) | `frame_x` / `frame_y` / `frame_width` / `frame_height` |
| `cw` / `ch` (`window_manager.c:2170-2171`) | `column_width` / `row_height` |
| `w` / `h` as a width/height field or parameter (`view.h:46-47`, `:53`, `:70`) | `width` / `height` |
| `x` / `y` as a coordinate | unchanged |
| `r` / `c` in `window_manager_apply_grid` (`window_manager.c:2124`) | `rows` / `columns` |
| `r` / `p` in `cgrect_contains_point` (`misc/helpers.h:558`) | `rect` / `point` |
| `t` / `p` in `triangle_contains_point` (`misc/helpers.h:564`) | `triangle` / `point` |
| `l1` / `l2` / `l3` (`misc/helpers.h:566-568`) | `first_edge_cross` / `second_edge_cross` / `third_edge_cross` |
| `r1` / `r2` / `r1_max` / `r2_max` (`view.c:541-582`) | `first_area` / `second_area` / `first_area_max_point` / `second_area_max_point` |
| `t` / `mt` (`window_manager.c:544-554`) | `interpolant` / `eased_interpolant` |
| `t` in the 21 easing functions (`misc/helpers.h:42-146`) | `interpolant` |
| `f` / `wp` / `c` in `mouse_determine_drop_action` (`mouse_handler.c:110-112`) | `destination_window_frame` / `point_relative_to_frame_origin` / `center_rect` |
| `t` / `r` / `b` / `l` in `mouse_determine_drop_action` (`mouse_handler.c:113-116`) | `top_triangle` / `right_triangle` / `bottom_triangle` / `left_triangle` |
| `cf` / `cs` (`window_manager.c:1893`) | `candidate_first_child_area` / `candidate_second_child_area` |
| `ca` (`window_manager.c:1896`) | `source_node_center_point` |
| `dcf` / `dcs` (`window_manager.c:1897-1898`) | `distance_to_candidate_first_child` / `distance_to_candidate_second_child` |
| `i` / `j` where an index is genuinely needed | `index` / `inner_index`; prefer an iterator |
| `n` as a count | `count` |
| `sr` / `sg` / `sb` / `sa` (`misc/helpers.h:632-635`) | `source_red` / `source_green` / `source_blue` / `source_alpha` |
| `one255` / `inv255` (`misc/helpers.h:601-602`) | unchanged — they are values, not abbreviations |
| `mask_ff` (`misc/helpers.h:604`) | unchanged |
| `fst` / `snd` (`message.c:445-446`) | `first_character` / `second_character` |
| `eui` (`misc/helpers.h:526`) | `enhanced_user_interface` |
| `mib` (`process_manager.c:76`) | `management_information_base` — the `sysctl` name array |
| `opt` (`yabai.c:244`) | `option` |
| `val` (`yabai.c:251`) | `value` |
| `argl` (`yabai.c:66`) | `argument_lengths` |
| `v` (`message.c:386`) | `value` |
| `dummy` (`sa.m:425`) | `dummy` — unchanged; it names the thing exactly |
| `win` (`window.c:1046`) | `is_window` |
| `fmt` (`message.c:418`, `:429`) | `format` |
| `tb` (`misc/timer.h:104`) | not translated — dead code |
| `sockfd` | `socket_file_descriptor` |
| `rsp` as a `FILE *` parameter | `response: &mut Response` |

---

## 11. Rust keyword collisions

| C spelling | Rust |
| --- | --- |
| `ref` | renamed to `element_ref` (or `handler_ref` on `CarbonProcessEventInstallation`, `observer_ref` where the C already says so). **No `r#ref` anywhere.** |
| `mod` | renamed to `event_modifier` or `misalignment`. **No `r#mod` anywhere.** |
| `type` | see the table below. **No `r#type` anywhere** — no field in the daemon genuinely names a type discriminator. |
| `move` | never appears as a C identifier; nothing to rename |

Every C `type` in `src/**`, and what it is called:

| site | C | Rust |
| --- | --- | --- |
| `struct event_signal::type` (`event_signal.h:96`) | `enum signal_type` | `PendingSignal::signal_type` |
| `struct token_value::type` (`message.c:273`) | `enum token_type` | `TokenValue::type_of_value` |
| `struct process_manager::type[3]` (`process_manager.h:22`) | `EventTypeSpec[3]` | `CarbonProcessEventInstallation::event_type` |
| `struct event::type` (`event_loop.h:57`) | `enum event_type` | **gone** — the `Event` variant is the tag |
| `event_loop_post` (`event_loop.h:74`) | `enum event_type type` | **gone** — the parameter is the whole `Event` |
| `event_signal_push` (`event_signal.h:119`) | `enum signal_type type` | `signal_type` |
| `event_signal_add` (`event_signal.h:122`) | `enum signal_type type` | `signal_type` |
| `event_signal_serialize` (`event_signal.c:400`) | `enum signal_type type` | `signal_type` |
| `parse_label` (`message.c:554`) | `enum label_type type` | `label_type` |
| `space_manager_set_layout_for_space` (`space_manager.h:74`) | `enum view_type type` | `view_type` |
| `space_manager_set_gap_for_space` (`space_manager.h:75`) | `int type` (`TYPE_ABS`/`TYPE_REL`) | `type_of_change` |
| `space_manager_set_padding_for_space` (`space_manager.h:87`) | `int type` | `type_of_change` |
| `window_manager_move_window_relative` (`window_manager.h:169`) | `int type` | `type_of_change` |
| `window_manager_adjust_window_ratio` (`window_manager.c:300`) | `int type` | `type_of_change` |
| `event_loop.c:971` | `int type = SLSSpaceGetType(...)` | `space_type` |
| `mouse_handler.c:70` | `int type = CGEventGetIntegerValueField(...)` | `gesture_type` |
| `MOUSE_HANDLER(name)` (`mouse_handler.h:21`) | `CGEventType type` | `event_type` |
| `connection_callback` (`misc/extern.h:1`) | `uint32_t type` | `notification_type` |
| `SLSNewWindow` / `SLSNewWindowWithOpaqueShapeAndContext` (`misc/extern.h:25-26`) | `int type` | `type_of_window` |
| `cfarray_of_cfnumbers` (`misc/helpers.h:344`) | `CFNumberType type` | `number_type` |

---

## 12. Spellings that are already correct and must not be "improved"

`connection`, `context`, `count`, `cursor`, `direction`, `flags`, `frame`, `gap`, `index`,
`info`, `label`, `layer`, `level`, `mask`, `mode`, `node`, `origin`, `parent`, `point`, `ratio`,
`root`, `size`, `split`, `type`, `value`, `view`, `window`, `zoom`, `uuid`, `opacity`, `easing`,
`proxy`, `image`, `alpha`, `radius`, `pitch`, `slide`, `order`, `rank`, `policy`, `terminated`.

Words that look like abbreviations but are the C's own vocabulary and stay: `app` in
`Rule`/`Signal`/`PendingSignal` and every `ARGUMENT_*` string, `mff`, `ffm`, `bsp`, `psn` inside
`psn_space_id`, `sa`/`osax`, `Src`/`Dst` in enum variants, `Pref` in `DockDidChangePref`,
`SHRT` in `*_OPT_SHRT`, `DPPM`/`REM`/`MOV`/`ANIM` in `OsaxAttrib`.

---

## 13. Conflicts this file settles

Every row below is a place where two phase-1 documents disagree. The **Binding** column is what
the crate uses; the other spelling is not written anywhere.

| Name | Binding | Rejected, and where it appears | Why |
| --- | --- | --- | --- |
| `view_flag` newtype | `ViewFlag` | `ViewFlags` — `patterns/state-and-ownership.md` §5.1 | `DECISIONS.md` 37: `enum foo_bar` → `FooBar`, singular, matching every other flag newtype |
| `window_node::insert_dir` | `insert_direction` | `insert_dir` — `patterns/state-and-ownership.md` §5.1 | `DECISIONS.md` 37 forbids abbreviated fields; the idioms glossary already fixed this row |
| `struct area` fields | `width` / `height` | `w` / `h` — `files/view-and-tests.md:101` | same |
| `view::insertion_point` | `WindowId` | "stored `enum window_insertion_point`" — `patterns/idioms-and-conventions.md` §5.1 | the C source: `view.c:646-647`, `window_manager.c:1793` assign a window id. `space_manager::window_insertion_point` is the enum |
| `struct window::ref` | `element_ref` | `reference` — `THREADS.md` §5.2 | the idioms glossary row for `ref` |
| `struct window::id_ptr` | field `liveness: Arc<WindowLivenessCell>` | field `id_pointer` — `patterns/idioms-and-conventions.md` §2.4 | `DECISIONS.md` 21 replaces the field outright; `id_pointer` survives only as the expansion for a local |
| process table static | `PROCESS_TABLE` | `PROCESS_MANAGER_TABLE` — `THREADS.md` §4 | `patterns/state-and-ownership.md` §1.2 is the globals authority |
| mouse tap static | `MOUSE_TAP_STATE` | `MOUSE_TAP_SHARED_STATE` — `THREADS.md` §4 | same |
| `struct window_animation_context` | `AnimationContext` | `WindowAnimationContext` (rule 1) | `DECISIONS.md` 24 writes `Arc<AnimationContext>` literally |
| `struct event_signal` | `PendingSignal` | `EventSignal` (rule 1) | `DECISIONS.md` 17 plus `patterns/state-and-ownership.md` §7 name the queued record `PendingSignal`; the local binding is still `event_signal` |
| `window_animation::window` + `::wid` | one field, `window_id` | two fields | `DECISIONS.md` 14: the pointer becomes the handle, and the handle is already there. One `DEVIATIONS.md` line |
| `Process::policy` | `AtomicI32`, explicitly initialised | uninitialised `i32` — `files/window-application-process.md` | `DECISIONS.md` 4. One `DEVIATIONS.md` line |
| flag sets | newtypes with associated consts | `bitflags!` — `files/window-application-process.md` | `DECISIONS.md` 11 and 31 |
| process table container | `Table<ProcessSerialNumber, Arc<Process>>` | `HashMap<PsnKey, *mut Process>` — `files/window-application-process.md` | `DECISIONS.md` 16 and 22 |
