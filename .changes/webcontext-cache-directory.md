---
"wry": patch
---

Set `WebsiteDataManagerBuilder::base_cache_directory` with the same path as `base_data_directory`.

This change allows the cache directory to be changed instead of using the default one [from WebKitGTK](https://webkitgtk.org/reference/webkit2gtk/stable/property.WebsiteDataManager.base-cache-directory.html).
