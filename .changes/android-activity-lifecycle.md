---
"wry": minor
---

Updated Android lifecycle JNI calls in `WryActivity` for Tao 0.36's renames:

> - `create` to `onFirstActivityCreate`
> - `onActivityCreate` to `onCreate`
> - `start` to `onStart`
> - `resume` to `onResume`
> - `pause` to `onPause`
> - `stop` to `onStop`
> - Removed `onActivitySaveInstanceState`
> - `onActivityDestroy` to `onDestroy`
> - `onActivityLowMemory` to `onLowMemory`
> 
> `onLowMemory` no longer takes any parameters.
> `onFirstActivityCreate` no longer takes any parameters.

and also emitting them as window-specific events.
