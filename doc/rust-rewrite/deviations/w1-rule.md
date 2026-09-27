# Deviations — `w1-rule` (W1-rule, `src/rule.h` + `src/rule.c` → `src/rule.rs`)

`src/rule.h:60` | `rule_clear_flag` was defined `static inline` but called from nowhere in `src/` | not translated (`DECISIONS.md` 5, `state-access/signal-rule-mouse.md` §6 and judgement call 16); `RuleFlag` carries `contains` and `insert` but no `remove`, while `RuleEffectsFlag` carries all three, and `src/rule.rs` declares fifteen functions where `TRANSLATION_PLAN.md` §3.3 sizes the unit at sixteen

`src/rule.c:6` | `rule_serialize` opened with `TIME_FUNCTION` | not translated (`DECISIONS.md` 5, the `PROFILE` machinery of `misc/timer.h`)

`src/rule.c:207-221` | `rule_destroy` freed the four `regex_t`s and the six `char *`s without clearing the flags or NULLing the pointers, so a second call was a double free | `impl Drop for Rule`, which makes a second destroy unrepresentable (`DECISIONS.md` 4, `patterns/message-and-serialisation.md` §10.3, `state-access/signal-rule-mouse.md` judgement call 11)

`src/rule.h:51-54`, `:10-13` | the four `regex_t` fields were paired with the `RULE_APP_VALID` / `RULE_TITLE_VALID` / `RULE_ROLE_VALID` / `RULE_SUBROLE_VALID` bits stored in `rule->flags`, and `rule_serialize` published those stored bits | four `Option<PosixRegex>` fields (`DECISIONS.md` 26, ruling 44, `GLOSSARY.md` §3.19); the four bits are no longer stored, and `rule_serialize` derives them from `Option::is_some` (`GLOSSARY.md` §5.3)

`src/message.c:2812`, `src/window_manager.c:174`, `:198` | `struct rule rule = {0}` and `struct rule_effects effects = {0}` zero-initialised the aggregates | `Rule` and `RuleEffects` derive `Default`, and `RuleFlag` / `RuleEffectsFlag` derive `Default` alongside the `Clone, Copy, PartialEq, Eq` of `GLOSSARY.md` §5 so that the two aggregates can derive it

`src/rule.h:56`, `:41` | `struct rule::flags` and `struct rule_effects::flags` were bare `uint16_t` | `flags: RuleFlag` and `flags: RuleEffectsFlag`, the newtypes of `GLOSSARY.md` §5.3 and §5.4, because the six `static inline` helpers become methods that hang off them; `GLOSSARY.md` §3.19 and §3.20 write the field type as "`u16`, tested with `RuleFlag`" and `state-access/signal-rule-mouse.md` judgement call 15 reads that as naming the newtype the field holds. `rule.c:60`'s `"flags":"0x%08x"` becomes `((effects.flags.0 as u32) << 16) | (rule.flags.0 as u32)`, byte-identical
