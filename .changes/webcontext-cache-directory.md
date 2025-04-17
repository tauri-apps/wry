---
"wry": minor
---

Breaking change: `WebContext::new` now takes a `cache_directory` argument besides `data_directory`.

This change allows users to specify a custom cache directory for the web context,
instead of using the default one [from WebKit2](https://webkitgtk.org/reference/webkit2gtk/stable/property.WebsiteDataManager.base-cache-directory.html).
