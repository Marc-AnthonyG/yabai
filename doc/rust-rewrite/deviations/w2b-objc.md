# Deviations — `w2b-objc` (bodies of `src/workspace.rs` and `src/sa.rs`)

## `src/workspace.rs`

`src/workspace.m:73`, `:83`, `:209-259` | the KVO `context` was the bare `struct process *`, owned by the process table and freed by `process_destroy`, while the `:WorstApiEverMade` blocks record that an observation can survive every `removeObserver:` (the exception is swallowed), so a later `-observeValueForKeyPath:` read `terminated` and `policy` through freed memory | each `addObserver:` mints one owned strong count with `Arc::into_raw(Arc::clone(process))`; every `removeObserver:` that returns without throwing (`workspace.m:57`, `:61`, `:94`, `:98`, `:227`, `:252`) retires exactly one count through `release_kvo_refcon_on_main_queue`, which drops it on the main queue; a removal that throws retires none (`DECISIONS.md` 4, 20, `THREADS.md` §6.3)

`src/workspace.m:107`, `:110`, `:216`, `:230` | `process->policy` was a plain `int` written and read from the main thread and the event-loop thread with no synchronisation | `Process::policy` is an `AtomicI32` accessed with `Relaxed` loads and stores, same values (`DECISIONS.md` 4, 47)

`src/workspace.m:213`, `:238` | `process->terminated` was read with a plain `volatile` load | `load(Ordering::Acquire)`, pairing with the release store at `src/process_manager.c:189` (`patterns/ffi-objc-and-os.md` §20.3)

`src/workspace.m:156` | `if ((self = [super init]))` guarded the eight registrations against a nil return from `-[NSObject init]`, which never returns nil | not translated as a branch: `msg_send![super(this), init]` returns `Retained<Self>`, so the nil branch is dead code (`DECISIONS.md` 5)

`src/workspace.m:125-138` | `workspace_display_notch_height` calls `[NSScreen screens]` and `-safeAreaInsets` — main-thread-only AppKit APIs — from the event-loop thread, through `display_bounds_constrained` (`src/display.c:141`) | the same calls from the same thread, with the `MainThreadMarker` `objc2-app-kit` demands minted on the spot by `MainThreadMarker::new_unchecked()` (`patterns/ffi-objc-and-os.md` §20.5, §28)

`src/workspace.m:129` | `__builtin_available(macos 12.0, *)` asked the running OS whether it is at least 12.0 | `!workspace_is_macos_bigsur()`, which admits exactly the same versions because the deployment target is 11.0 and the six flags are written at `src/yabai.c:295`, before any display is measured (`patterns/ffi-objc-and-os.md` §20.5)

## `src/sa.rs`

`src/sa.m:252-257` | the NUL scan over the zero-initialised `char rsp[BUFSIZ]` always stops inside the buffer (`recv` fills at most `BUFSIZ - 1` bytes), but `memcpy(attrib, zero+1, sizeof(uint32_t))` reads up to four bytes past the end of `rsp` when the first NUL sits at index `BUFSIZ - 4` or later | the NUL is scanned over the whole buffer exactly as in C, so a short reply still yields the zero bytes C read from the initialised tail; the four attribute bytes are bounds-checked against the buffer and the out-of-bounds case fails the handshake (`result` stays `false`) instead of reading past it (`DECISIONS.md` 4). This replaces the `:252-257` wording of `deviations/w1-sa.md`, whose "within the bytes actually received" check would have rejected short replies that C accepts without undefined behaviour

`src/sa.m:80-93`, `:126-135`, `:158`, `:196` | `snprintf` into a `char[MAXLEN]` cuts the text at 511 bytes, possibly in the middle of a multi-byte UTF-8 sequence | `truncate_as_snprintf_would_into_a_maxlen_buffer` cuts at 511 bytes too, backing off to the preceding character boundary because a `String` cannot hold a partial sequence; only a path or user name longer than 511 bytes with a multi-byte character straddling byte 511 is affected

`src/sa.m:324` | `strnstr(bootargs, "-arm64e_preview_abi", len)` | `libc` 0.2.189 declares no `strnstr`, so the same search runs over `bootargs[..len]` cut at its first NUL, which is exactly the range `strnstr` examines (`patterns/ffi-objc-and-os.md` §16.2)
