# Signature changes — `w2-window-part1` (part of W2-window)

No frozen signature was changed, and no other module was asked for one. Three items were **added**
to `src/window.rs`, all private to the module except the first:

`src/window.h:24-29` | `static CFStringRef ax_window_notification[]` | `pub(crate) fn ax_window_notification() -> &'static [CFStringOwned; 3]` | the skeleton stores the three notification names in `AX_WINDOW_NOTIFICATION: OnceLock<[CFStringOwned; 3]>` and nothing initialises it; the accessor is the `get_or_init` that both `window_observe` and `window_unobserve` read through, and it matches the `ax_application_notification` naming of `DECISIONS.md` 50
`src/window.c:21-29` | — | `struct WindowUnobserveRequest` | the payload `window_unobserve` hands to the main queue, named as in `THREADS.md` §5.4: the `+1` observer and element references, the notification bits and the refcon `Arc` pointer
`src/window.c:21-29` | — | `fn window_unobserve_on_main_queue(request: *mut WindowUnobserveRequest)` | the trampoline of `THREADS.md` §5.4; it is reached through `dispatch_after_on_main_queue(0, …)`, the only dispatch wrapper `DECISIONS.md` 42 provides
