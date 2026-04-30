---
"wry": patch
---

On macOS, avoid an extra copy for owned custom protocol response bodies of 128KB or larger by transferring the body buffer into `NSData`.
