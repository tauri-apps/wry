---
"wry": minor
---

**Breaking change**: Upgrade `ndk` crate to `0.9` and delete unused `ndk-sys` and `ndk-context` dependencies.  Types from the `ndk` crate are used in public API surface.
**Breaking change**: The public `android_setup()` function now takes `&ThreadLooper` instead of `&ForeignLooper`, signifying that the setup function must be called on the thread where the looper is attached (and the `JNIEnv` argument is already thread-local as well).
