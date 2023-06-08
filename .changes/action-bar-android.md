---
"wry": "patch"
---

Remove ActionBar handling from wry. If you want to hide the action bar, hide it using the `themes.xml` file in your android project or inherit `WryActivity` class and use `getSupportActionBar()?.hide()` in the `onCreate` method.
