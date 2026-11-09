# Wake lock: brief

Keep the screen or the machine awake while the app needs it, and release it reliably.

## Options

- keepawake: Rust, desktop, keeps the whole machine awake; release on drop
- screen-wake-lock: Rust, desktop display sleep; young crate
- Native: UIApplication isIdleTimerDisabled on iOS; FLAG_KEEP_SCREEN_ON on Android

## Recommended path

keepawake on desktop. Native idle-timer and window flags on mobile. Acquire a guard and release it on drop.

## Edge cases to design for

- A lock held after a crash keeps the device awake; tie it to a scope
- iOS idle timer changes do not apply while the app is backgrounded
- Android flags apply to the window, not the app
- Battery impact: keep the lock as short as the task allows

## Research status

Research done 2026-11-09 by web search. Versions and licences are unverified until checked on the registry before pinning. Full catalogue: the research notes for the SuperNSH library set.
