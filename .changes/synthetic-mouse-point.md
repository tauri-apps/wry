---
"wry": patch
---

On macOS and Linux, the synthetic `mousedown`/`mouseup` of a mouse's back and forward buttons now carries the pointer's point in CSS pixels, and no longer throws (`null is not an object (evaluating 'el.dispatchEvent')` on macOS) where that point stands past the page's edge, as under a page zoom: macOS converts the point by the view's `isFlipped` and `pageZoom`, Linux by the view's zoom level unless it zooms text only, and an event with no element under it goes to the document element.
