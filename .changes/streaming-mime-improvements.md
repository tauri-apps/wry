---
"wry": patch
---

Improve the `streaming` example with MIME type auto-detection and a remote HTTPS test tab.

**MIME type auto-detection (all platforms)**

`stream_protocol` previously served every local file as `video/mp4`, causing playback
to fail for WebM, Ogg, and other formats. It now detects the correct MIME type from
the file extension:

| MIME type | Extensions |
|-----------|-----------|
| `video/mp4` | `.mp4` `.m4v` `.m4p` |
| `video/webm` | `.webm` |
| `video/ogg` | `.ogg` `.ogv` |
| `video/quicktime` | `.mov` `.qt` |
| `video/x-matroska` | `.mkv` `.mk3d` |
| `video/x-msvideo` | `.avi` |
| `video/x-flv` | `.flv` `.f4v` |
| `video/x-ms-wmv` | `.wmv` `.asf` |
| `video/mpeg` | `.mpeg` `.mpg` `.mpe` `.m2v` `.m1v` |
| `video/mp2t` | `.ts` `.m2ts` `.mts` |
| `video/3gpp` | `.3gp` `.3gpp` |
| `video/3gpp2` | `.3g2` `.3gpp2` |
| `video/hevc` | `.hevc` `.h265` |
| `application/mxf` | `.mxf` |
| `application/vnd.rn-realmedia` | `.rm` `.rmvb` |

**Remote HTTPS test tab (all platforms)**

`examples/streaming/index.html` now has a *Local file / Remote HTTPS* toggle. The
remote tab lets you pick a format (MP4/H.264, MP4/AV1, WebM/VP9) and resolution
(360p, 720p, 1080p) and streams a 10-second Big Buck Bunny clip from
[test-videos.co.uk](https://test-videos.co.uk) directly over HTTPS — no local file
needed and the `stream://` custom protocol is not involved.

*Big Buck Bunny* (c) 2008 Blender Foundation, licensed under CC BY 3.0.
