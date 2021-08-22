---
"wry": patch
---

Linux port can now access to file protocol. We open this because we couldn't get the header with custom protocols. So it
will take huge chunk of memory loading large files. Able to access file protocol on linux could bypass this. Note that
other platforms still can't access to file protocol.
