---
"wry": minor
---

On Android, `WryActivity.startActivity` can start the activity in a task of its own (`FLAG_ACTIVITY_NEW_DOCUMENT | FLAG_ACTIVITY_MULTIPLE_TASK`) and defaults to the launcher activity. It fails right away, with an error naming the activity, when the launch mode would hand the intent to an existing instance. The new `closeActivity` finishes the activity, and its task when it is the task root. tao uses both to open each window in a separate task.

A `WryActivity` that another app starts (a deep link, a notification, a share) while a window is open now hands its intent to the most recently resumed window, brings that window's task to the front and finishes, instead of staying open as a blank screen that the next window would claim. This happens with launch mode `standard`, and with `singleInstancePerTask` below Android 12 (API 31), which treats that mode as `standard`.
