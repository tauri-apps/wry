---
"wry": minor
---

On Android, `WryActivity.startActivity` can start the activity in a task of its own (`FLAG_ACTIVITY_NEW_DOCUMENT | FLAG_ACTIVITY_MULTIPLE_TASK`) and defaults to the launcher activity. It fails right away, with an error naming the activity, when the launch mode would hand the intent to an existing instance. The new `closeActivity` finishes the activity, and its task when it is the task root. tao uses both to open each window in a separate task.
