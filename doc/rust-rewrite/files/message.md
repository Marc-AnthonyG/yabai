# `src/message.c` + `src/message.h` -> Rust

Phase-1 mapping. Source read in full: `src/message.c` (3045 lines), `src/message.h` (7 lines).
All `path:line` references are against the tree at commit `dd84572`.

---

## 1. Purpose

`message.c` is the entire command surface of the yabai daemon. It owns (a) a hand-rolled tokenizer
over the NUL-separated argv blob that the `yabai -m ...` client writes into the unix socket, (b) the
full domain/command/selector dispatch tree (`config`, `display`, `space`, `window`, `query`, `rule`,
`signal`) that mutates the global managers and writes text back into a `FILE *` response stream, and
(c) the listening socket plus the dedicated accept thread that feeds connections to the event loop.

`message.h` exposes only two symbols: `handle_message(FILE *rsp, char *message)` and
`message_loop_begin(char *socket_path)`. Everything else in the file is `static`.

---

## 2. Types: structs, enums, unions, typedefs, `#define` constants, X-macro lists

### 2.1 `struct token` — `src/message.c:254`

```c
struct token { char *text; int length; };
```

| field    | C type  | ownership                                                                                 |
| -------- | ------- | ----------------------------------------------------------------------------------------- |
| `text`   | `char *` | **Borrowed, never owned.** Points into the request buffer allocated by `ts_alloc_unaligned` in `EVENT_HANDLER(DAEMON_MESSAGE)` (`src/event_loop.c:1622`). That arena is reset by `ts_reset()` right after the handler returns (`src/event_loop.c:1672`), so no token may outlive one `handle_message` call. |
| `length` | `int`   | byte count, not including the terminating NUL.                                              |

Two invariants the whole file silently depends on:

1. `text` is **always NUL-terminated at `text[length]`** — because the wire protocol separates
   arguments with NUL bytes (`src/yabai.c:74-81`). This is why `sscanf(value.text, ...)`,
   `strtof(token.text, ...)` and `string_equals(key, ...)` are legal on a "slice".
   The one exception is the `stack.` prefix strip at `src/message.c:1060-1064`, which shortens
   `length` by advancing `text` — the NUL still sits at `text[length]`.
2. `text` is **mutable**; two functions write NUL bytes into it in place (see §6 P8).

Not shared across threads: a token only exists on the event-loop pthread stack.

### 2.2 `enum token_type` — `src/message.c:260`

`TOKEN_TYPE_INVALID, TOKEN_TYPE_UNKNOWN, TOKEN_TYPE_INT, TOKEN_TYPE_FLOAT, TOKEN_TYPE_U32,
TOKEN_TYPE_STRING`. `TOKEN_TYPE_UNKNOWN` is unreachable in practice (the `string_value` branch at
`src/message.c:408` can only be false if `token.text` were NULL, which `token_is_valid` already
excluded).

### 2.3 `struct token_value` — `src/message.c:270`

```c
struct token_value {
    struct token token;
    enum token_type type;
    union { int int_value; float float_value; uint32_t u32_value; char *string_value; };
};
```

A classic tagged union. `token` stays valid regardless of `type` and is used for every error
message (`value.token.length`, `value.token.text`). The union arms alias: `token_to_value`
(`src/message.c:397`) writes `int_value`, then `u32_value`, then `float_value`, then
`string_value` into the *same* storage as it walks the type chain, so for a `TOKEN_TYPE_STRING`
result the numeric arms hold the low 4 bytes of a pointer. Every consumer reads only the arm
matching `type`. `string_value` is borrowed (it is literally `token.text`, `src/message.c:408`).

### 2.4 `enum label_type` — `src/message.c:508`

`LABEL_DISPLAY, LABEL_SPACE, LABEL_WINDOW`. Selects which reserved-identifier table `parse_label`
checks against.

### 2.5 `struct properties` — `src/message.c:605`

```c
struct properties { struct token token; bool did_parse; bool did_error; uint64_t flags; };
```

`token` is the token that was inspected; when `did_parse == false` the caller re-uses that token as
the *next* token instead of pulling a fresh one (`src/message.c:2428`, `2484`, `2541`). `flags` is a
bitmask OR-ed from `display_property_val` / `space_property_val` / `window_property_val`.
`did_error` is set independently of `did_parse` and aborts the query. Nothing is owned.

### 2.6 `struct selector` — `src/message.c:652`

```c
struct selector {
    struct token token;
    bool did_parse;
    union { int dir; uint32_t did; uint64_t sid; struct window *window; };
};
```

Second tagged union — except the tag is implicit in *which parse function produced it*. The union
is zero-initialized by the designated initializer `{ .token = ..., .did_parse = true }` in all four
producers, so `did`/`sid`/`window`/`dir` read as 0/NULL when the selector parsed but failed to
resolve (e.g. `north` with no display to the north: `did_parse` stays `true`, `did` stays `0`).
Every call site therefore tests `selector.did_parse && selector.<arm>`.
`window` is a **borrowed** pointer into `g_window_manager`'s window hashtable; it is not owned and
must not outlive the current event-loop iteration.

### 2.7 `#define` constant tables

These are pure string constants, never variables. They are the CLI grammar.

* Domains — `src/message.c:14-20`: `DOMAIN_CONFIG` "config", `DOMAIN_DISPLAY` "display",
  `DOMAIN_SPACE` "space", `DOMAIN_WINDOW` "window", `DOMAIN_QUERY` "query", `DOMAIN_RULE` "rule",
  `DOMAIN_SIGNAL` "signal".
* Config commands — `src/message.c:23-54` (33 entries, `COMMAND_CONFIG_DEBUG_OUTPUT` ..
  `COMMAND_CONFIG_SKIP_SPACE_ANIMATION`).
* Config selector — `src/message.c:56`: `SELECTOR_CONFIG_SPACE` "--space".
* Config arguments — `src/message.c:58-89` (31 entries). Note `ARGUMENT_CONFIG_EXTERNAL_BAR`
  (`src/message.c:89`) is not a keyword but a **`sscanf` format string** `"%5[^:]:%d:%d"`.
* Display commands — `src/message.c:93-95`.
* Space commands — `src/message.c:99-114`; space arguments `src/message.c:116-127`, of which
  `ARGUMENT_SPACE_PADDING` `"%255[^:]:%d:%d:%d:%d"` and `ARGUMENT_SPACE_GAP` `"%255[^:]:%d"` are
  `sscanf` formats.
* Window commands — `src/message.c:131-150`; window arguments `src/message.c:152-179`, of which
  `ARGUMENT_WINDOW_GRID` `"%d:%d:%d:%d:%d:%d"`, `ARGUMENT_WINDOW_MOVE` / `ARGUMENT_WINDOW_RESIZE`
  `"%255[^:]:%f:%f"` and `ARGUMENT_WINDOW_RATIO` `"%255[^:]:%f"` are `sscanf` formats.
* Query commands/arguments — `src/message.c:183-189`.
* Rule commands/arguments — `src/message.c:193-216`. `ARGUMENT_RULE_VALUE_SPACE` is a **char**
  literal `'^'` (`src/message.c:215`), `ARGUMENT_RULE_VALUE_GRID` is a `sscanf` format
  (`src/message.c:216`).
* Signal commands/arguments — `src/message.c:220-232`.
* Common arguments — `src/message.c:236-251`, including `ARGUMENT_COMMON_SEL_STACK_PREFIX`
  `"stack."` which is used with `token_prefix`, not `token_equals`.

### 2.8 X-macro lists consumed (defined elsewhere, listed because phase 2 must keep them in sync)

| list | defined at | used in message.c at |
| --- | --- | --- |
| `DISPLAY_PROPERTY_LIST` -> `display_property_val[]`, `display_property_str[]` | `src/display.h:7-35` | `src/message.c:2425` |
| `SPACE_PROPERTY_LIST` -> `space_property_val[]`, `space_property_str[]` | `src/view.h:21-40` | `src/message.c:2481` |
| `WINDOW_PROPERTY_LIST` -> `window_property_val[]`, `window_property_str[]` | `src/window.h:66-85` | `src/message.c:2538` |
| `ANIMATION_EASING_TYPE_LIST` -> `animation_easing_type_str[]`, `EASING_TYPE_COUNT` | `src/misc/helpers.h:27-40` | `src/message.c:1318-1323` |
| `EVENT_TYPE_LIST` -> `enum event_type` (`DAEMON_MESSAGE`) | `src/event_loop.h:6-53` | `src/message.c:3009` |

Enum-to-string tables read by `handle_domain_config` when a value is omitted:
`bool_str` (`src/misc/helpers.h:173`), `ffm_mode_str` (`src/window_manager.h:47`),
`display_arrangement_order_str` (`src/display_manager.h:15`),
`window_origin_mode_str` (`src/window_manager.h:61`), `window_node_child_str` (`src/view.h:115`),
`window_insertion_point_str` (`src/view.h:101`), `animation_easing_type_str`
(`src/misc/helpers.h:35`), `purify_mode_str` (`src/window_manager.h:33`),
`view_type_str` (`src/view.h:177`), `window_node_split_str` (`src/view.h:130`),
`auto_balance_str` (`src/view.h:138`), `mouse_mod_str` (`src/mouse_handler.h:83`),
`mouse_mode_str` (`src/mouse_handler.h:93`), `external_bar_mode_str` (`src/display_manager.h:29`).

Helper macros used: `array_count` (`src/misc/macros.h:8`), `in_range_ii` / `in_range_ei`
(`src/misc/macros.h:12,14`), `FAILURE_MESSAGE` = `"\x07"` (`src/misc/macros.h:18`),
`MAXLEN` = 512 (`src/misc/macros.h:20`), `DIR_NORTH/EAST/SOUTH/WEST` (`src/misc/macros.h:26-29`),
`TIME_FUNCTION` (`src/misc/timer.h:136`).

---

## 3. Globals and statics

### 3.1 `g_message_loop` — `src/message.c:1-5` (file-scope, anonymous struct)

```c
static struct { int sockfd; bool is_running; pthread_t thread; } g_message_loop;
```

| field        | type        | initial      | written by                     | read by                      | synchronisation |
| ------------ | ----------- | ------------ | ------------------------------ | ---------------------------- | --------------- |
| `sockfd`     | `int`       | 0 (BSS)      | main thread, `message_loop_begin` `src/message.c:3023` | message-loop pthread, `src/message.c:3006` | **none** — relies on the write happening before `pthread_create` at `src/message.c:3042` (the create is the happens-before edge). |
| `is_running` | `bool`      | false (BSS)  | main thread, `src/message.c:3041` | message-loop pthread, `src/message.c:3005` | **none**; never set back to `false` anywhere in the tree (verified: the only references are the eight lines listed by grep). The loop therefore runs until process exit. |
| `thread`     | `pthread_t` | 0 (BSS)      | `src/message.c:3042`            | nothing                      | n/a; the thread is never joined and never detached. |

### 3.2 `static const int token_char_int_table[]` — `src/message.c:283-296`

Sparse designated-initializer table sized by its highest index `'f'` == 102, so 103 `int`s, all
zero except the 22 hex-digit slots. Read-only, indexed only after an explicit range check
(`src/message.c:349`, `src/message.c:372-374`). Touched by the event-loop pthread only.

### 3.3 Reserved-identifier tables

* `static char *reserved_display_identifiers[]` — `src/message.c:515-527`, 10 entries.
* `static char *reserved_space_identifiers[]` — `src/message.c:529-537`, 6 entries.
* `static char *reserved_window_identifiers[]` — `src/message.c:539-552`, 11 entries.

All point at string literals (static storage, never freed). Read at `src/message.c:571`, `579`,
`587` and again at `src/message.c:2623` (from `parse_rule`). Event-loop pthread only.

### 3.4 `extern` globals reached from this file — `src/message.c:7-12`

All defined in `src/yabai.c` and part of the unity build's single translation unit.

| symbol | type | what message.c does with it |
| --- | --- | --- |
| `g_event_loop` | `struct event_loop` | `event_loop_post` from the **message-loop pthread** (`src/message.c:3009`) — lock-free MPSC push (`src/event_loop.c:1684-1703`). |
| `g_display_manager` | `struct display_manager` | reads `last_display_id` (`:761`), `order` (`:1208`), `mode`/`top_padding`/`bottom_padding` (`:1692`); **writes** `order` (`:1210,1212,1214`) and `mode`/`top_padding`/`bottom_padding` (`:1674-1687`). |
| `g_space_manager` | `struct space_manager` | reads `last_space_id` (`:842`), `window_placement`, `window_insertion_point`, `window_zoom_persist`, `skip_window_focus_animation`, `top/bottom/left/right_padding`, `window_gap`, `layout`, `split_ratio`, `split_type`, `auto_balance`; **writes** `window_placement` (`:1236,1238`), `window_insertion_point` (`:1247,1249,1251`), `window_zoom_persist` (`:1260,1262`), `skip_window_focus_animation` (`:1271,1273`), `split_ratio` (`:1547`). |
| `g_window_manager` | `struct window_manager` | reads `enable_mff`, `ffm_mode`, `window_origin_mode`, `enable_window_opacity`, `window_opacity_duration`, `window_animation_duration`, `window_animation_easing`, `purify_mode`, `menubar_opacity`, `active_window_opacity`, `normal_window_opacity`, `insert_feedback_color`; **writes** `enable_mff` (`:1186,1188`), `window_origin_mode` (`:1223,1225,1227`), `window_opacity_duration` (`:1293`), `window_animation_duration` (`:1303,1307`), `window_animation_easing` (`:1323`), `insert_feedback_color` (`:1375`). |
| `g_mouse_state` | `struct mouse_state` | **writes** `modifier` (`:1623-1631`), `action1` (`:1640,1642`), `action2` (`:1651,1653`), `drop_action` (`:1662,1664`) — see the cross-thread note below. |
| `g_verbose` | `bool` | read `:1173`, **written** `:1175,1177`. |

**Cross-thread fields.** `g_mouse_state.modifier` is declared `volatile uint8_t`
(`src/mouse_handler.h:71`) and `action1` / `action2` / `drop_action` are plain enums
(`src/mouse_handler.h:68-72`). They are written here on the **event-loop pthread** and read by the
`CGEventTap` callback, which fires on the **main run loop thread** (the tap's run loop source is
added to `CFRunLoopGetMain()` at `src/mouse_handler.c:288`). There is no lock, no atomic, no
fence. `g_verbose` is likewise written here and read by `debug()` from every thread.
The three managers are conceptually event-loop-owned but are also read by main-thread AX/SLS
callbacks; that is the daemon's pre-existing design and this file does not add synchronisation.

There are **no function-local statics** in `message.c`.

---

## 4. Functions

Thread determination, once, for the whole file:

* `handle_message` (`src/message.c:2979`) is declared in `src/message.h:4` and has exactly **one**
  caller in the daemon: `src/event_loop.c:1634`, inside `EVENT_HANDLER(DAEMON_MESSAGE)`. That
  handler is dispatched from `event_loop_run` (`src/event_loop.c:1646`), the pthread started by
  `event_loop_begin`. (The unrelated `handle_message` at `src/osax/payload.m:957` is a different,
  static function inside the injected payload and is out of scope.) **Every `static` parse/dispatch
  function below is reachable only through `handle_message`, therefore all of them run on the
  event-loop pthread.**
* `message_loop_begin` (`src/message.c:3016`) has one caller: `src/yabai.c:344`, in `main`, before
  `[NSApp run]` — **main thread**.
* `message_loop_run` (`src/message.c:3003`) is the pthread start routine created at
  `src/message.c:3042` — its own **message-loop pthread**.

Unless stated otherwise a function allocates nothing and calls no libc beyond what `daemon_fail`
already does.

### Tokenizer

| fn | signature | what it does | allocates |
| --- | --- | --- | --- |
| `get_token` `:298` | `static struct token get_token(char **message)` | Advances the cursor to the next NUL, returns the span as a borrowed token, then steps past the NUL **only if the following byte is non-NUL** — so at the double-NUL terminator the cursor parks forever and every further call returns a zero-length token. | nothing |
| `token_prefix` `:317` | `static bool token_prefix(struct token, char *match)` | True when `match` is a prefix of the token (also true on exact equality). | nothing |
| `token_equals` `:327` | `static bool token_equals(struct token, char *match)` | Exact byte equality with a NUL-terminated literal. Returns `true` for a zero-length token against `""`. | nothing |
| `token_is_valid` `:338` | `static inline bool token_is_valid(struct token)` | `text != NULL && length > 0`. | nothing |
| `token_is_positive_integer` `:343` | `static bool token_is_positive_integer(struct token, int *value)` | Digits-only decimal accumulate into a signed `int`. Always writes `*value` (0 first), even on failure. | nothing |
| `token_is_hexadecimal` `:358` | `static bool token_is_hexadecimal(struct token, uint32_t *value)` | Requires `0x`/`0X` and at least one digit (`length <= 2` rejected); accumulates into `uint32_t`. | nothing |
| `token_is_float` `:383` | `static bool token_is_float(struct token, float *value)` | `strtof(token.text, &end)`; success only when the **whole** C string was consumed. **External: `strtof` (libc).** | nothing |
| `token_to_value` `:397` | `static struct token_value token_to_value(struct token)` | Type-tags a token: int -> u32 -> float -> string, first match wins. | nothing |

### Response writers

| fn | signature | notes |
| --- | --- | --- |
| `daemon_fail` `:418` | `static inline void daemon_fail(FILE *rsp, char *fmt, ...)` | No-op when `rsp == NULL` (this is how `parse_*_selector(NULL, ...)` silences probing parses). Writes the single BEL byte `FAILURE_MESSAGE` then the formatted text. **External: `va_start`, `va_end`, `fprintf`, `vfprintf` (libc).** Not marked `__printf__`, so the compiler does not check the format. |
| `daemon_deprecated` `:429` | `__unused static inline void daemon_deprecated(FILE *rsp, char *fmt, ...)` | Dead today; kept deliberately with `__unused`. Prefix `"deprecation warning: "`, **no** BEL. |

### Value parsers

| fn | signature | what it does | notes |
| --- | --- | --- | --- |
| `parse_key_value_pair` `:440` | `static void parse_key_value_pair(char *token, char **key, char **value, bool *exclusion)` | Splits `k=v` / `k!=v` **in place** by writing a NUL over the `=` (or over the `!`). On failure sets `*key`/`*value` to NULL and **leaves `*exclusion` untouched** (callers pre-init it to `false`). `k=` (empty value) yields `key` set, `value` NULL, and the buffer unmodified. |
| `parse_value_type` `:472` | `static uint8_t parse_value_type(char *type)` | `"abs"` -> `TYPE_ABS`, `"rel"` -> `TYPE_REL`, else `0`. **External: `strcmp` via `string_equals` (`src/misc/helpers.h:254`).** |
| `parse_resize_handle` `:483` | `static uint8_t parse_resize_handle(char *handle)` | 9-way map to `HANDLE_*` bit flags; `top_left` etc. are OR-combinations. Else `0`. |
| `parse_label` `:554` | `static bool parse_label(FILE *rsp, struct token, enum label_type, char **label)` | Invalid token -> `*label = NULL`, returns `true` (this is the "remove the label" path). Non-string token or reserved keyword -> `daemon_fail`, returns `false`. Otherwise **`malloc(length+1)` + `memcpy` + manual NUL**; the caller hands ownership to a manager. Returns `false` silently (no message) if `malloc` fails. **External: `malloc`, `memcpy`.** |
| `parse_property` `:613` | `static inline bool parse_property(struct properties *, char *property, uint64_t *property_val, char **property_str, int property_count)` | Linear scan of the property-name table, OR-ing the matched value into `properties->flags`. |
| `parse_properties` `:625` | `static struct properties parse_properties(FILE *rsp, struct token, uint64_t *property_val, char **property_str, int property_count)` | Splits a comma list **in place** (`token.text[i] = '\0'` at `:637`). Only runs when the token is valid and does not start with `--`. A trailing comma is an error (the `i+1 == token.length` branch is tested first). Error message length is `i-cursor+1`. |

### Selector parsers

All four are `static struct selector parse_*(FILE *rsp, char **message, <acting>, bool optional)`
(except `parse_insert_selector`, which takes no acting value and no `optional`). All run
`TIME_FUNCTION` (except `parse_insert_selector`). `optional` only suppresses the failure message
for the `TOKEN_TYPE_INVALID` case; every other failure still reports.

| fn | signature | yabai symbols called |
| --- | --- | --- |
| `parse_display_selector` `:665` | `(FILE*, char**, uint32_t acting_did, bool optional)` | `display_manager_arrangement_display_id`, `display_manager_find_closest_display_in_direction`, `display_manager_prev_display_id`, `display_manager_next_display_id`, `display_manager_first_display_id`, `display_manager_last_display_id`, `display_manager_cursor_display_id`, `display_manager_get_display_for_label` |
| `parse_space_selector` `:790` | `(FILE*, char**, uint64_t acting_sid, bool optional)` | `space_manager_mission_control_space`, `space_manager_prev_space`, `space_manager_next_space`, `space_manager_first_space`, `space_manager_last_space`, `space_manager_cursor_space`, `space_manager_get_space_for_label` |
| `parse_window_selector` `:871` | `(FILE*, char**, struct window *acting_window, bool optional)` | `window_manager_find_window`, `..._find_closest_managed_window_in_direction`, `..._find_window_below_cursor`, `..._find_largest/smallest_managed_window`, `..._find_sibling/first_nephew/second_nephew/uncle/first_cousin/second_cousin_for_managed_window`, `..._find_prev/next/first/last/recent_managed_window`, `..._find_prev/next/first/last/recent_window_in_stack`, `..._find_window_in_stack`. **External: `strlen` (libc) at `:1063-1064`.** |
| `parse_insert_selector` `:1130` | `(FILE*, char**)` | none; maps `north/east/south/west/stack` to `DIR_*` / `STACK`. |

### Domain handlers

| fn | signature | one-liner | allocations / ownership | external (non-yabai) symbols |
| --- | --- | --- | --- | --- |
| `handle_domain_config` `:1152` | `static void handle_domain_config(FILE *rsp, struct token domain, char *message)` | Optional `--space <SPACE_SEL>` prefix, then a **loop** over chained `key [value]` commands; with no value it prints the current setting, otherwise it sets it. | none of its own | `fprintf`, `sscanf` (libc); `CGPreflightScreenCaptureAccess`, `CGRequestScreenCaptureAccess` (CoreGraphics) at `:1306,1310` |
| `handle_domain_display` `:1700` | `(FILE *rsp, struct token domain, char *message)` | Optional display selector, then exactly **one** of `--focus` / `--space` / `--label` (no loop). | `parse_label` malloc handed to `display_manager_set_label_for_display` (`src/display_manager.c:61`), which takes ownership (`src/display_manager.c:74-77`) | none |
| `handle_domain_space` `:1759` | `(FILE *rsp, struct token domain, char *message)` | Optional space selector, then a **loop** over `--focus/--switch/--move/--swap/--display/--create/--destroy/--equalize/--balance/--mirror/--rotate/--padding/--gap/--toggle/--layout/--label`. `--create`/`--destroy` can rebind `acting_sid` for later iterations. | `parse_label` malloc handed to `space_manager_set_label_for_space` (`src/space_manager.c:183`), which takes ownership | `sscanf` |
| `handle_domain_window` `:2047` | `(FILE *rsp, struct token domain, char *message)` | Optional window selector, then a **loop** over 20 commands. A per-iteration guard (`:2063-2071`) rejects a missing acting window for everything except `--focus/--close/--minimize/--deminimize/--toggle`. | `parse_label` malloc handed to `window_manager_set_scratchpad_for_window`; **leaks on failure**, see §6 T13 | `sscanf` |
| `handle_domain_query` `:2419` | `(FILE *rsp, struct token domain, char *message)` | `--displays` / `--spaces` / `--windows`, each with an optional comma property list and an optional `--display`/`--space`/`--window` narrowing selector. | none | `fprintf` |
| `parse_rule` `:2596` | `static bool parse_rule(FILE *rsp, char **message, struct rule *rule, struct token token)` | Consumes the remaining tokens as `key[!]=value` pairs into a `struct rule`. Requires at least one of `app=`/`title=`/`role=`/`subrole=` (`has_filter`). Reports at most one unsupported `!` (the last one seen). | **Owns via `string_copy` (malloc)**: `rule->label`, `rule->app`, `rule->title`, `rule->role`, `rule->subrole`, `rule->effects.scratchpad`. **Owns four `regex_t`** compiled in place. All freed by `rule_destroy` (`src/rule.c:207`). | `regcomp` + `REG_EXTENDED` (libc `regex.h`), `sscanf`, `malloc`/`memcpy`/`strlen` via `string_copy` (`src/misc/helpers.h:397`) |
| `handle_domain_rule` `:2806` | `(FILE *rsp, struct token domain, char *message)` | `--add [--one-shot]`, `--apply`, `--remove`, `--list`. | `--add`: on success `rule_add` (`src/rule.c:175`) **bitwise-copies the whole `struct rule`, regex_t included, into `g_window_manager.rules`** and the local is deliberately *not* destroyed; on failure `rule_destroy`. `--apply` with an unmatched label parses a throwaway rule and always destroys it (`:2837`). | none |
| `handle_domain_signal` `:2864` | `(FILE *rsp, struct token domain, char *message)` | `--add` (key/value loop building a `struct signal`), `--remove`, `--list`. Requires `event=` and `action=`. | **Owns via `string_copy`**: `signal.label`, `signal.app`, `signal.title`, `signal.command`; **owns two `regex_t`**. Handed to `event_signal_add` on success, `event_signal_destroy` on failure. | `regcomp`, `malloc` via `string_copy` |
| `handle_message` `:2979` | `void handle_message(FILE *rsp, char *message)` | Pulls the first token and dispatches to one of the seven domain handlers, else `daemon_fail("unknown domain ...")`. | nothing | none |

### Socket / thread

| fn | signature | thread | notes |
| --- | --- | --- | --- |
| `message_loop_run` `:3003` | `static void *message_loop_run(void *context)` | **message-loop pthread** (created `:3042`) | `while (is_running) { accept(); if (-1) continue; event_loop_post(&g_event_loop, DAEMON_MESSAGE, NULL, sockfd); }`. `context` is unused — hence the `#pragma clang diagnostic` pair at `:3001`/`:3014`. On a persistent `accept` error this busy-spins. The accepted fd is handed to the event loop as the bare `int param1`; the **event loop** then owns it and closes it (`fclose` at `src/event_loop.c:1636` or `socket_close` at `:1642`). **External: `accept` (BSD sockets).** |
| `message_loop_begin` `:3016` | `bool message_loop_begin(char *socket_path)` | **main thread** (`src/yabai.c:344`) | `unlink` -> `socket(AF_UNIX, SOCK_STREAM)` -> `bind` -> `chmod 0600` -> `listen(SOMAXCONN)` -> `fcntl(F_SETFD, FD_CLOEXEC \| F_GETFD)` -> set `is_running` -> `pthread_create`. Returns `false` on any failure **without closing `sockfd`** (`src/yabai.c:345-346` then calls `error()` which exits, so it does not matter in practice). `struct sockaddr_un socket_address;` at `:3018` is **not** zero-initialized — `sun_len` and the trailing padding are garbage passed to `bind`. `snprintf` into `sun_path` silently truncates at 104 bytes. **External: `snprintf`, `unlink`, `socket`, `bind`, `chmod`, `listen`, `fcntl`, `pthread_create`.** |

---

## 5. Callbacks registered with the OS or a run loop

`message.c` registers **exactly one** OS-level callback, and it is a thread entry point, not a
run-loop or notification callback.

| | |
| --- | --- |
| registration site | `src/message.c:3042` — `pthread_create(&g_message_loop.thread, NULL, &message_loop_run, NULL)` |
| callback | `message_loop_run` (`src/message.c:3003`) |
| thread it fires on | a brand-new pthread, referred to throughout this document as the message-loop pthread |
| context pointer | `NULL`. The parameter is declared and ignored; the file suppresses the warning with `#pragma clang diagnostic ignored "-Wunused-parameter"` at `src/message.c:3001-3014`. |
| lifetime | for the life of the process: `is_running` is never cleared, the thread is never joined or detached, and there is no teardown function. |

Everything the thread produces crosses to the event loop as a plain `int` file descriptor inside a
`struct event` (`src/message.c:3009` -> `src/event_loop.c:1684`), which is a lock-free CAS push onto
an intrusive singly linked list followed by `sem_post`.

There are **no** `AXObserver` callbacks, `CGEventTap` callbacks, Carbon event handlers,
`SLSRegisterConnectionNotifyProc` procs, `NSNotification` observers, dispatch blocks,
`CVDisplayLink` callbacks or signal handlers in this file. (The `CGEventTap` whose configuration
`handle_domain_config` mutates lives in `src/mouse_handler.c:278-288`.)

---

## 6. C pattern catalogue -> recommended Rust

Patterns from the standard checklist that are **absent** from this file, so phase 2 should not go
looking for them: `goto`-out cleanup, stretchy buffers (`buf_push`/`buf_len`/`buf_del`), intrusive
linked lists, hash tables, arena allocation (`ts_alloc*` — the request buffer is allocated by the
caller in `src/event_loop.c:1622`, never here), `CFRetain`/`CFRelease` pairs, Objective-C message
sends or blocks, `fork`/`exec`, `setjmp`, CAS on `id_ptr` as a liveness check, inline asm, SIMD.

### P1 — Pointer-walking tokenizer with a `char **` cursor

`src/message.c:298-315`; every `get_token(&message)` call site (`:1157`, `:1166`, `:1169`,
`:1171`, ... `:2981`).

**Rust.** A cursor over the request bytes, not a raw pointer:

```rust
struct MessageCursor<'a> { bytes: &'a mut [u8], at: usize }
struct Token { start: usize, len: usize }   // indices into MessageCursor::bytes
```

Return index pairs rather than `&[u8]`, because `parse_key_value_pair` and `parse_properties`
need `&mut` access to the same buffer while a token is alive — borrowck will reject a
`&'a [u8]` token co-existing with a `&'a mut [u8]` write. Indices sidestep that without `unsafe`.

**Do not silently change:** the advance rule at `src/message.c:308-312`. `get_token` steps past the
terminating NUL *only when the next byte is non-NUL*, so at the double-NUL the cursor stops moving
and every subsequent call yields a zero-length token. A naive `split(|b| *b == 0)` iterator would
instead yield an infinite tail of empty items **or** stop early; the loops at `:1169`, `:1779`,
`:2062`, `:2604`, `:2877` all terminate on `token_is_valid` being false, so the "parks forever"
behaviour is load-bearing and must be reproduced exactly.
Also note `src/message.c:308` reads `(*message)[1]`, **one byte past the current NUL**. With a
well-formed client message (`src/yabai.c:81` appends the second NUL) that is in bounds; a malformed
message would read past the end of the `ts` arena into its guard page. In Rust, bounds-check and
treat an out-of-range index as "end of message" rather than panicking.

### P2 — Borrowed slice struct pointing into a caller-owned arena

`struct token` (`src/message.c:254`), `selector.window` (`src/message.c:661`),
`token_value.string_value` (`src/message.c:279`).

**Rust.** Lifetimes give this for free. `handle_message` becomes
`pub fn handle_message(response: &mut dyn Write, message: &mut [u8])` and every token borrows from
`message`. The arena reset (`ts_reset()` at `src/event_loop.c:1672`) becomes the end of the
borrow — nothing to do. `selector.window` becomes an index/id into the window table rather than a
`&Window`, because the selector is stored across calls that also mutate the window manager
(`src/message.c:2078`, `:2094`, `:2112`); a `&Window` would not survive borrowck. Use
`Option<WindowId>` (`u32`) and re-look-up, or a `Rc`/arena index depending on what the
`window_manager` module lands on.

### P3 — Tagged unions

`struct token_value` (`src/message.c:270-281`), `struct selector` (`src/message.c:652-663`).

**Rust.** Real enums, not `union`:

```rust
enum TokenValue<'a> {
    Invalid,
    Unknown,
    Int(i32),
    Float(f32),
    U32(u32),
    String(&'a [u8]),
}
struct ParsedTokenValue<'a> { token: Token, value: TokenValue<'a> }
```

and four distinct selector result types (`DisplaySelector`, `SpaceSelector`, `WindowSelector`,
`InsertSelector`), each carrying `token: Token` and its own `Option<T>` payload instead of the
implicit `did_parse && arm != 0` convention. Be careful to keep the *tri-state*: C distinguishes
"did not parse at all" (`did_parse == false`) from "parsed but did not resolve"
(`did_parse == true`, arm == 0) and the two take different branches at `src/message.c:1708-1713`,
`:1767-1772`, `:2055-2060`, `:1861-1867`, `:2076-2082`. Model it as
`struct Selector<T> { token: Token, parsed: bool, value: Option<T> }` or a three-variant enum —
not as a bare `Option`.

### P4 — Sparse designated-initializer lookup table

`token_char_int_table` (`src/message.c:283-296`).

**Rust.** `const TOKEN_CHAR_INT_TABLE: [u8; 103] = { ... }` built in a `const` block, or simply
`(c as char).to_digit(16)`. The table is only ever reached after an explicit ASCII range check, so
`to_digit(16)` is behaviourally identical and clearer. No `unsafe`.

### P5 — Manual integer accumulation with wrapping overflow

`src/message.c:343-356` (`*value = *value * 10 + ...` on a signed `int`),
`src/message.c:358-381` (`*value = *value * 16 + ...` on `uint32_t`).

**Rust.** `i32::wrapping_mul` / `wrapping_add` and `u32::wrapping_mul` / `wrapping_add`.

**Do not silently change:** signed overflow in the decimal path is UB in C but wraps in the
optimised build, and `u32` overflow in the hex path wraps by definition. `yabai -m window --focus
99999999999` today wraps to some `i32` and then fails the window lookup. Rust's `*` panics in debug
and wraps in release — use the explicit `wrapping_*` so debug and release agree with C.
Also keep the "always write `*value`, even on failure" behaviour (`src/message.c:345`,
`src/message.c:360`): `token_to_value` relies on the union arm being written before the next
candidate overwrites it, and `src/message.c:1101` calls `token_is_positive_integer` for its bool
only.

### P6 — `strtof` used as a whole-string float validator

`src/message.c:383-395`.

**Rust.** This is the single most dangerous spot in the file for a naive port.
`strtof` accepts, and `str::parse::<f32>()` rejects: leading whitespace (`" 1.5"`), and hex float
literals (`"0x1p3"` -> 8.0 — reachable, because `token_is_hexadecimal` rejects it at
`src/message.c:372-374` on the `p`). Both accept `+1.0`, `inf`, `infinity`, `nan`, `1e5`, and both
are case-insensitive for those words. `strtof` additionally accepts `nan(n-char-seq)`.
Recommended: call `libc::strtof` through FFI for an exact match, or write an explicit
strtof-compatible front end. If phase 2 chooses `str::parse`, note in the module that `"0x1p3"` and
leading-whitespace values change from `TOKEN_TYPE_FLOAT` to `TOKEN_TYPE_STRING`.
Also note: because the int branch is tried first, `"5"` is `INT` and `"-5"` is `FLOAT` — negative
integers arrive as floats everywhere (this is why `window --move rel:-100:0` works through the
`%f` `sscanf` formats and not through the int path).

### P7 — varargs formatting into a `FILE *` response

`daemon_fail` (`src/message.c:418-427`), `daemon_deprecated` (`src/message.c:429-438`), and ~40
direct `fprintf(rsp, ...)` calls.

**Rust.** The `FILE *` becomes a buffered writer over the socket. The daemon-side stream is created
by `fdopen(param1, "w")` at `src/event_loop.c:1633` and closed with `fflush`+`fclose` at
`src/event_loop.c:1636-1637`, so the Rust shape is
`let mut response = BufWriter::new(unsafe { UnixStream::from_raw_fd(fd) });` with an explicit
`flush()` before drop. Replace varargs with `std::fmt::Arguments`:

```rust
fn daemon_fail(response: Option<&mut dyn Write>, args: std::fmt::Arguments);
// or a macro: daemon_fail!(response, "unknown domain '{}'\n", ...)
```

**Keep `rsp == NULL` as `Option`.** `daemon_fail` early-returns on a null `rsp`
(`src/message.c:420`), and `handle_domain_display`/`_space`/`_window`/`_query` deliberately pass
`NULL` to the first, probing `parse_*_selector` call (`src/message.c:1706`, `:1765`, `:2053`) so
that a leading command token is not reported as a bad selector. That must survive as
`Option<&mut dyn Write>`; a `&mut dyn Write` to a sink would change nothing observable but an
eager `unwrap` would break the probe.

**`FAILURE_MESSAGE` is one BEL byte** (`src/misc/macros.h:18`) written *before* the text, and the
client tests only `rsp[0]` of each `read()` chunk (`src/yabai.c:111`). A message that triggers
several `daemon_fail` calls emits several BELs mid-stream — preserve that, do not hoist the prefix.

### P8 — In-place mutation of the request buffer to split strings

`src/message.c:464` (`*token = '\0'` over `=` or `!`), `src/message.c:637`
(`token.text[i] = '\0'` over each `,`).

**Rust.** Both writers need `&mut [u8]`. Either keep the in-place splitting on a `&mut [u8]`
(faithful, and it keeps the NUL-termination invariant that downstream `sscanf`-shaped parsing
relies on), or return index ranges and stop mutating. Prefer **returning ranges**: nothing outside
these two functions observes the mutation, and it removes the only reason `handle_message` needs
`&mut [u8]` rather than `&[u8]`. If phase 2 keeps `&mut`, the `Token { start, len }` design of P1
is mandatory.

### P9 — Out-parameters

`parse_key_value_pair(char*, char**, char**, bool*)` (`:440`), `token_is_positive_integer(_, int*)`
(`:343`), `token_is_hexadecimal(_, uint32_t*)` (`:358`), `token_is_float(_, float*)` (`:383`),
`parse_label(_, _, _, char **label)` (`:554`).

**Rust.** Return values. `parse_key_value_pair` -> `Option<KeyValue<'a>>` where
`struct KeyValue<'a> { key: &'a [u8], value: &'a [u8], exclusion: bool }`; the C failure path sets
key and value to NULL and *does not touch* `exclusion` (`src/message.c:461-463`), which the caller
compensates for by pre-initialising it to `false` (`src/message.c:2607`, `:2880`) — returning
`None` reproduces that exactly. `parse_label` -> `Result<Option<String>, ()>`, where `Ok(None)` is
the "clear the label" path (`src/message.c:558-561`).

### P10 — String-literal command table + long `token_equals` if/else-if chains

The whole dispatch: `handle_domain_config` `:1170-1696` (33 arms), `handle_domain_space`
`:1780-2043` (16 arms), `handle_domain_window` `:2073-2415` (20 arms), plus the selector parsers.

**Rust.** `match` on the token bytes: `match token_bytes { b"--focus" => ..., b"--close" => ..., _
=> daemon_fail!(...) }`. Byte-literal patterns keep the comparison byte-exact (no UTF-8 validation,
no locale) and match `token_equals` semantics exactly. Keep the `const` names
(`COMMAND_WINDOW_FOCUS` etc.) as `const COMMAND_WINDOW_FOCUS: &[u8] = b"--focus";` so the grammar
stays greppable and phase 3 can split the tables per domain module.

**Do not silently change:** the *order* of the arms is observable where prefixes overlap. In
`parse_window_selector` the `stack.` **prefix** test at `:1060` comes after every exact-match arm,
and in `handle_domain_window --toggle` the final arm at `:2332` is a fallthrough that treats any
unrecognised value as a scratchpad label
(`window_manager_toggle_scratchpad_window_by_label(&g_window_manager, value.text)`) and only fails
if that returns false. A `match` with a `_` arm reproduces this; a `HashMap` dispatch would not.

### P11 — Reserved-identifier arrays + `array_count`

`src/message.c:515-552`, iterated at `:571`, `:579`, `:587`, `:2623`.

**Rust.** `const RESERVED_WINDOW_IDENTIFIERS: [&[u8]; 11] = [...]` and `.iter().any(...)`. Note
`src/message.c:2623-2627` compares with `string_equals(value, ...)` (a NUL-terminated `char*`) while
`:587-589` compares with `token_equals` (a length-bounded token) — the same table, two comparison
styles. In Rust both become byte-slice equality; make sure the `parse_rule` side compares the value
slice, not the whole key/value token.

### P12 / P13 — `malloc`-ed owned C strings handed to a manager

`parse_label` (`src/message.c:596-601`) and `string_copy` (`src/misc/helpers.h:397`) at
`src/message.c:2618`, `:2631`, `:2639`, `:2649`, `:2659`, `:2669`, `:2891`, `:2893`, `:2901`,
`:2923`.

**Rust.** `String` (or `Vec<u8>`, since the source bytes are not validated as UTF-8 anywhere —
`String::from_utf8_lossy` would alter a label containing invalid UTF-8). Recommend `Vec<u8>` for
`rule.app`/`title`/`role`/`subrole` (they are regex source text and window titles can be arbitrary
bytes) and `String` only where the value is already constrained.

Ownership transfers to reproduce:
`display_manager_set_label_for_display` (`src/display_manager.c:61`) stores the pointer
(`:74-77`), `space_manager_set_label_for_space` (`src/space_manager.c:183`) stores it (`:196-199`),
`window_manager_set_scratchpad_for_window` (`src/window_manager.c:2492`) stores it on success.
In Rust these become moves and all three become obviously correct.

### P14 — Bit-flag accumulation

`properties->flags |= property_val[i]` (`src/message.c:617`), `rule_set_flag` /
`rule_effects_set_flag` (`src/rule.h:57-64`) at `src/message.c:2640`, `:2642`, `:2650`, `:2652`,
`:2660`, `:2662`, `:2670`, `:2672`, `:2682`, `:2696`, `:2719`, `:2762`, `:2765`, `:2768`, `:2771`,
`:2816`; `view_set_flag` at `:1386`, `:1409`, `:1432`, `:1455`, `:1478`, `:1502`, `:1511`, `:1520`,
`:1558`, `:1561`, `:1564`, `:1589`, `:1592`, `:1595`, `:1598`.

**Rust.** `bitflags` crate, or a newtype over `u64`/`u16` with `const` associated values and
`BitOrAssign`. The property flags are a `u64` (`src/display.h:23`) and rule flags a `u16`
(`src/rule.h:54`) — keep the widths, because `window_property_val` already uses bit `0x100000000`
(`src/window.h:64`), which does not fit in 32 bits.

### P15 — `sscanf` with `%N[^:]` scansets into fixed stack buffers

`src/message.c:1670-1672` (`char mode[6]`, `"%5[^:]:%d:%d"`),
`:1970-1972` (`char type[MAXLEN]`, `"%255[^:]:%d:%d:%d:%d"`),
`:1981-1983` (`"%255[^:]:%d"`),
`:2218-2220` (six `unsigned` via `"%d:%d:%d:%d:%d:%d"`),
`:2230-2232` and `:2242-2244` (`"%255[^:]:%f:%f"`),
`:2258-2260` (`"%255[^:]:%f"`),
`:2708-2711` (six `unsigned` via `"%d:%d:%d:%d:%d:%d"`),
`:2718` (`"%f"`).

**Rust.** Hand-rolled splitters, one per format, e.g.

```rust
fn parse_colon_type_and_two_floats(text: &[u8]) -> Option<(&[u8], f32, f32)>
```

**Four things a naive `split(':')` + `parse()` gets wrong:**

1. **Trailing junk is ignored by `%d`/`%f`.** `sscanf("abs:1:2:3:4xyz", "%255[^:]:%d:%d:%d:%d")`
   returns 5 and succeeds; `"4xyz".parse::<i32>()` fails. Use a `strtol`/`strtof`-style
   *longest-valid-prefix* parse per field.
2. **The scanset needs at least one character.** `"%255[^:]"` against `":1:2:3:4"` matches nothing,
   so `sscanf` returns 0, not 5 — an empty type field is a *parse failure*, which falls through to
   the `daemon_fail` branch. `"".split(':')` would hand you an empty first element instead.
3. **`%5[^:]` truncates.** `src/message.c:1670` caps the external-bar mode at 5 characters, so
   `external_bar mainxxxx:0:0` yields `mode == "mainx"`, which then fails all three
   `string_equals` tests at `:1673/:1678/:1683` and reports "unknown mode 'mainx'". Cap the slice
   at 5 bytes to reproduce the message text.
4. **Only the full field count is accepted.** Every call site tests `== N` and takes the
   `daemon_fail` branch otherwise; the partially-assigned stack variables are never read. Return
   `Option` and require all fields.

**Also:** `src/message.c:2220` and `src/message.c:2708` pass `unsigned *` to a `%d` conversion — a
formal type mismatch that works because the widths match. In Rust parse as `i32` and cast
`as u32`, so a negative grid component keeps its two's-complement value rather than failing.

`value.text` at each of these sites may be the **empty string** when the token is invalid (the
cursor parks on the final NUL, P1); `sscanf("")` returns `EOF` (-1), which is `!= N`, so
`external_bar` with no value falls through to *printing* the current setting at
`src/message.c:1692`. Reproduce: empty input must take the "print current value" path, not error.

### P16 — `printf` formatting of the response

`%.*s` with `(token.length, token.text)` at ~60 sites; `%s` from the enum-name tables; `%d`, `%f`,
`%.4f`, `0x%x`, `%s:%d:%d`.

**Rust.**

* `%.*s` -> write the raw bytes: `response.write_all(&bytes[token.start..token.start+token.len])`.
  Do **not** route through `str`/`String::from_utf8_lossy` — a window title or label with invalid
  UTF-8 would gain replacement characters in the error text. Build a small
  `struct TokenBytes<'a>(&'a [u8])` with a `Display`-like writer, or format the message in two
  writes.
* `%f` -> `{:.6}` **after casting to `f64`** (C promotes the `float` argument to `double` in the
  varargs call). `%.4f` -> `{:.4}` on an `f64`. Rust's default `{}` for `f32` prints the shortest
  round-trip form (`0.35` vs C's `0.350000`) — a visible protocol change for
  `yabai -m config window_opacity_duration` and friends. Affected lines: `:1291`, `:1300`
  (`%f`); `:1346`, `:1355`, `:1364`, `:1545` (`%.4f`).
* `0x%x` (`:1373`) -> `{:#x}` or `format!("0x{:x}", ...)` on the `u32` `insert_feedback_color.p`
  (`src/misc/helpers.h:164`).
* `%d` on `int` -> `{}` on `i32`. The enum-table lookups (`bool_str[g_verbose]`, `view_type_str[...]`)
  become `match`es or `const` arrays indexed by the enum's discriminant — in Rust prefer a method
  on the enum so an out-of-range discriminant is impossible.

### P17 — POSIX ERE via `regcomp` stored by value

`src/message.c:2641`, `:2651`, `:2661`, `:2671` (into `struct rule`, `src/rule.h:48-51`);
`src/message.c:2895`, `:2903` (into `struct signal`, `src/event_signal.h:109-110`).
Freed by `regfree` in `rule_destroy` (`src/rule.c:209-212`) and `event_signal_destroy`.

**Rust — this is a real decision, not a mechanical port.** `regcomp(..., REG_EXTENDED)` is POSIX
ERE. The Rust `regex` crate is *not* POSIX ERE: it accepts `\d`, `\w`, `\b`, `\p{...}`, non-greedy
`*?`, and `(?i)` flags, none of which POSIX ERE defines; conversely POSIX ERE treats `\d` as a
literal `d` in most implementations. Existing `.yabairc` files were written against POSIX
semantics. Two options:

* **Exact (recommended):** FFI to `libc::regcomp` / `regexec` / `regfree`, wrapped in a
  `struct PosixRegex(libc::regex_t)` with a `Drop` impl calling `regfree`. `unsafe` is confined to
  that one type. `regex_t` must be boxed or pinned — the C code *moves* it (see P18/T16) and
  `regex_t` is documented as movable, but keeping it behind a `Box` removes the question.
* **Divergent:** the `regex` crate, and document the change. Patterns that are valid in both and
  mean the same thing are the common case, so most configs would be unaffected — but the failure
  mode is silent (a rule stops matching), which is the worst kind.

Note `regcomp` is called **without** `REG_NOSUB`; matching happens in `src/window_manager.c` and
`src/event_signal.c`, so whatever type phase 2 picks must be shared with those modules.

### P18 — Zero-initialized aggregate moved by value into a container

`struct rule rule = {0}` (`src/message.c:2812`, `:2833`), `struct signal signal = {0}`
(`src/message.c:2875`), moved by `rule_add` (`src/rule.c:175-179`, a bitwise `buf_push` of the whole
struct) and `event_signal_add`.

**Rust.** `Rule::default()` / `Signal::default()` and `rules.push(rule)`. The move semantics are
native. **Critical:** the C success path at `src/message.c:2820-2821` deliberately does *not* call
`rule_destroy`, because `rule_add` took the owned pointers and the `regex_t`s by copy; the failure
path at `:2823` does. In Rust this becomes "move into the Vec" vs "drop" and is automatic — but
phase 2 must not add a `rule_destroy`-equivalent after `rule_add`, and must not make `Rule: Copy`.

### P19 — Two-phase parse, then commit or destroy

`src/message.c:2820-2824` (rule), `src/message.c:2954-2958` (signal), and the `rule_apply` +
unconditional `rule_destroy` at `src/message.c:2834-2837`.

**Rust.** Build the value, then `if did_parse { manager.add(value) }` — the `else` branch
disappears because `Drop` handles it. Keep the *partial* semantics though: `parse_rule` keeps going
after an error (`continue` at `src/message.c:2613`, `did_parse = false` without an early return),
so several errors are reported in one response and the partially-built rule is still destroyed.
A Rust `?`-based early return would emit only the first error. Use an
`errors_seen: bool` accumulator exactly as the C does.

### P20 — Error accumulation with a borrowed "last offender" pointer

`char *unsupported_exclusion = NULL` (`src/message.c:2600`, `:2870`), assigned at 13 sites, read at
`:2798-2801` and `:2949-2952`.

**Rust.** `let mut unsupported_exclusion: Option<&[u8]> = None;`. Note it records only the **last**
offending key, so `yabai -m rule --add app=X display!=1 space!=2` reports `space` only. Preserve.

### P21 — `enum` error codes matched with else-if chains

`enum space_op_error` and `enum window_op_error` results, matched at `src/message.c:1733-1741`,
`:1784-1792`, `:1798-1806`, `:1812-1822`, `:1828-1836`, `:1842-1856`, `:1870-1878`, `:1891-1903`,
`:2120-2126`, `:2134-2138`, `:2163-2175`, `:2181-2193`, `:2199-2205`, `:2211-2215`, `:2222-2224`,
`:2234-2236`, `:2246-2252`, `:2262-2266`.

**Rust.** `match` on the error enum. The C chains all fall through silently on the success variant
(there is no `else`), so the Rust `match` needs an explicit `_ => {}` or `Ok(_) => {}`. Note that
several of these chains do **not** handle every variant of the enum — e.g. `:2222-2224` handles only
`WINDOW_OP_ERROR_INVALID_SRC_VIEW` out of `window_manager_apply_grid`'s possible results. An
exhaustive Rust `match` would force phase 2 to invent messages; use a catch-all instead and keep the
message set identical.

### P22 — Unsynchronised cross-thread writes to config fields

`src/message.c:1623-1631` writes `g_mouse_state.modifier`, declared `volatile uint8_t`
(`src/mouse_handler.h:71`); `:1640-1664` write `action1`, `action2`, `drop_action`, plain enums
(`src/mouse_handler.h:68-72`). All four are read from the `CGEventTap` callback on the **main run
loop thread** (`src/mouse_handler.c:278-288`). `g_verbose` (`src/yabai.c:51`) is written at
`src/message.c:1175-1177` and read by `debug()` from every thread.

**Rust.** `AtomicU8` for `modifier` (the `volatile` is exactly an attempt at a relaxed atomic),
`AtomicU8` or `AtomicUsize` for the three `mouse_mode` enums with `From`/`TryFrom` conversions, and
`AtomicBool` for `g_verbose`. `Ordering::Relaxed` reproduces the C. Do **not** wrap the whole
`mouse_state` in a `Mutex` — the event tap callback runs in the input path and the C never blocks
there. This is the one place in `message.c` where a "safe Rust" translation must consciously pick
atomics rather than `&mut`.

### P23 / P24 — Global singletons and a file-scope anonymous-struct singleton

`extern` managers (`src/message.c:7-12`) and `static struct { ... } g_message_loop`
(`src/message.c:1-5`).

**Rust.** `g_message_loop` has no reason to be global once `message_loop_begin` returns a value:
the only fields read after startup are `sockfd` and `is_running`, both only by the spawned thread.
Translate to

```rust
pub fn message_loop_begin(socket_path: &Path) -> std::io::Result<JoinHandle<()>>
```

with the listener **moved into the thread closure**. That removes the global entirely and makes the
`pthread_create` happens-before edge explicit. The managers stay global for phase 2 (they are the
subject of other mapping files); whatever they land on — `OnceLock<Mutex<...>>`, a context struct
threaded through, or `thread_local` — `handle_message` should take them as parameters rather than
reaching for globals, so phase 3 can split this file without re-plumbing.

### P25 — BSD socket setup + accept loop handing an `int` fd to another thread

`src/message.c:3016-3044` and `:3003-3013`.

**Rust.**

```rust
let _ = std::fs::remove_file(socket_path);              // unlink, :3021
let listener = UnixListener::bind(socket_path)?;        // socket+bind, :3023-3029
std::fs::set_permissions(socket_path, Permissions::from_mode(0o600))?; // chmod, :3031
// listen(SOMAXCONN) is implicit in UnixListener::bind
```

`FD_CLOEXEC` (`src/message.c:3039`) is **already set by Rust** on sockets created through
`UnixListener`, so that line has no translation. The accept loop becomes
`for stream in listener.incoming()`, and the fd handed to the event loop becomes
`stream.into_raw_fd()` (ownership moves to the event-loop side, matching
`src/event_loop.c:1633-1642` which `fclose`s or `socket_close`s it).

**Do not silently change:**

* `src/message.c:3018` leaves `struct sockaddr_un` **uninitialized** apart from `sun_family` and
  `sun_path`; `sun_len` and the tail padding are garbage passed to `bind` with
  `sizeof(socket_address)`. `UnixListener::bind` zero-fills and passes the exact length. Same
  observable result, different bytes on the syscall — harmless, but worth knowing if anyone diffs
  syscall traces.
* `snprintf` into `sun_path` (`src/message.c:3020`) **silently truncates** at 104 bytes, so an
  over-long `$USER` today produces a listener on a truncated path. `UnixListener::bind` returns
  `Err(InvalidInput)` instead. This is a behaviour change; given the path format
  (`src/yabai.c:86`) it is unreachable in practice, but it is the kind of thing that should be a
  deliberate note rather than a surprise.
* `accept` returning -1 is `continue`d (`src/message.c:3007`) — a hard busy-spin under `EMFILE`.
  `listener.incoming()` yields `Err` items; `continue` on `Err` reproduces the C exactly. Do not
  "improve" it into a `break` or a backoff without saying so.
* The bind/chmod/listen failure paths return `false` **without closing** the fd
  (`src/message.c:3027-3037`). Rust's `?` drops the listener and closes it. Benign divergence
  (`src/yabai.c:345` exits immediately either way).

### P26 / P27 — `#pragma clang diagnostic` and `__unused`

`src/message.c:3001-3014` suppresses `-Wunused-parameter` for `message_loop_run`'s ignored
`context`; `src/message.c:429` marks `daemon_deprecated` `__unused`.

**Rust.** `_context` naming, and `#[allow(dead_code)]` on the `daemon_deprecated` equivalent. Keep
`daemon_deprecated` — it is deliberately retained infrastructure, and deleting it would be a
judgement call phase 2 should not make on its own.

### P28 — `TIME_FUNCTION`: `__attribute((cleanup))` RAII by macro

Used at `src/message.c:667`, `:792`, `:873`, `:1154`, `:1702`, `:1761`, `:2049`, `:2421`, `:2598`,
`:2808`, `:2866`. Expands (`src/misc/timer.h:136-138`) to a `cleanup`-attributed local plus a
`BEGIN_TIME_BLOCK` call, and compiles to **nothing** unless the profiler build flag is on
(`src/misc/timer.h:151`, `:157`).

**Rust.** A guard struct with a `Drop` impl, or just omit it. Note `g_profiler`
(`src/misc/timer.h`) is a plain global with no synchronisation and `__COUNTER__`-assigned anchor
indices — a translation-unit-wide mechanism that does not survive the split into crates intact.
Recommendation: drop `TIME_FUNCTION` from `message` in phase 2 and let whoever owns the profiler
module decide (a `#[cfg(feature = "profile")]` macro would be the direct equivalent). Flag it
rather than silently losing it.

### P29 — Re-running the token scanner on an inner substring

`src/message.c:2685` and `:2699`: `parse_display_selector(rsp, &value, ...)` /
`parse_space_selector(rsp, &value, ...)` where `value` is a `char *` pointing *inside* the message
buffer (the right half of a `key=value` pair that `parse_key_value_pair` already split).

**Rust.** The selector parsers must take a generic cursor, not a "whole message" type:
`fn parse_display_selector(response: Option<&mut dyn Write>, cursor: &mut MessageCursor, ...)`, and
`parse_rule` constructs a second `MessageCursor` over the value sub-slice. Because `get_token`
advances the *local* `value` pointer, the outer message cursor is untouched — a second cursor value
reproduces this. Note the `^` prefix handling immediately before
(`src/message.c:2680-2683`, `:2694-2697`): `ARGUMENT_RULE_VALUE_SPACE` is the char `'^'`, and when
present it is consumed (`++value`) and sets `RULE_FOLLOW_SPACE` **before** the selector parse.

### P30 — Loop-with-reassignment dispatch

`for (; token_is_valid(command); command = get_token(&message))` at `src/message.c:1169`, `:1779`,
`:2062`; `for (; token_is_valid(token); token = get_token(message))` at `:2604`;
`for (struct token token = get_token(&message); token_is_valid(token); token = get_token(&message))`
at `:2877`.

**Rust.** `while let Some(command) = cursor.next_token_if_valid() { ... }`.
**Do not silently change:** the loops in `handle_domain_space` and `handle_domain_window` can
**rebind the acting target mid-loop** (`acting_sid` at `src/message.c:1863`, `:1884`;
`acting_window` at `:2056`, `:2078`, `:2094`, `:2112`), so later commands in the same message act on
the new target. And several arms `return` out of the whole handler rather than `break`
(`:1865`, `:1886`, `:2080`, `:2096`, `:2114`, `:2375`, `:2390`), abandoning every remaining command
silently. Both are observable through `yabai -m window --focus next --close`-style chains.

### P31 — Prefix strip by pointer arithmetic on a borrowed slice

`src/message.c:1060-1064`: `result.token.text += strlen("stack."); result.token.length -= ...`.

**Rust.** `let rest = &token_bytes[b"stack.".len()..];` — but note the *side effect*: the C mutates
`result.token`, and that shortened token is what the failure message at `src/message.c:1110` prints
(re-prefixed manually with `ARGUMENT_COMMON_SEL_STACK_PREFIX` in the format string). Keep both the
strip and the manual re-prefix so the error text is byte-identical.

---

## 7. Existing comments to carry over verbatim

`message.c` contains exactly **one** explanatory comment. Everything else is a section banner.

### Explanatory comment (must survive into Rust)

| line | text |
| --- | --- |
| `src/message.c:311` | `// NOTE(asmvik): don't go past the null-terminator` |

It sits in the `else` branch of `get_token` (`src/message.c:308-312`) — the branch is otherwise
empty and exists purely to carry the note. In Rust the branch disappears, so attach the comment to
the condition that implements it.

### Section banners (`/* ... */`)

These delimit the `#define` groups. They carry no information beyond the grouping, and phase 3
turns each group into its own module, at which point the banner becomes the module name. Carry them
through phase 2 as-is if the constants stay in one file; drop them in phase 3 when the split makes
them redundant.

| lines | banner |
| --- | --- |
| `:22`, `:90` | `DOMAIN CONFIG` open / close |
| `:92`, `:96` | `DOMAIN DISPLAY` open / close |
| `:98`, `:128` | `DOMAIN SPACE` open / close |
| `:130`, `:180` | `DOMAIN WINDOW` open / close |
| `:182`, `:190` | `DOMAIN QUERY` open / close |
| `:192`, `:217` | `DOMAIN RULE` open / close |
| `:219`, `:233` | `DOMAIN SIGNAL` open / close |
| `:235`, `:252` | `COMMON ARGUMENTS` open / close |

The closing banners are all the identical string
`/* ----------------------------------------------------------------------------- */`.

**No other comments exist in the file.** Any comment appearing in the Rust translation that is not
on this list is new and should not be there.

---

## 8. Latent defects found while reading (carry the behaviour, or fix deliberately)

These are places where the mechanical Rust translation changes behaviour *for the better*. Each one
should be a conscious decision in phase 2, not an accident.

* **`src/message.c:2404`** — `yabai -m window --scratchpad <label>` leaks the `malloc`-ed label when
  `window_manager_set_scratchpad_for_window` returns `false`. Compare
  `src/window_manager.c:161-163`, which frees it on the same failure. Rust `String` ownership fixes
  this silently.
* **`src/message.c:2637-2676`** — a repeated rule key (`app=A app=B`) overwrites `rule->app` and
  `rule->app_regex`, leaking the previous `string_copy` and the previously compiled `regex_t`.
  Assigning to an `Option<Vec<u8>>` / `Option<PosixRegex>` in Rust drops the old value. Same class
  of silent fix.
* **`src/message.c:3027-3037`** — `sockfd` is leaked on the bind/chmod/listen failure paths.
* **`src/message.c:3018`** — `struct sockaddr_un` is passed to `bind` with uninitialized `sun_len`
  and padding.
* **`src/message.c:308`** — `(*message)[1]` reads one byte past the current token's NUL; a
  malformed client message (missing the second NUL that `src/yabai.c:81` appends) reads past the end
  of the `ts` arena allocation.
* **`src/message.c:2220`, `:2708`** — `%d` conversions writing into `unsigned` lvalues.
* **`src/message.c:1101`** — `token_is_positive_integer(result.token, &index)` is called after
  `token_is_valid(result.token)` in a `&&` chain, but `index` is only read when both succeed;
  correct as written, just non-obvious.
