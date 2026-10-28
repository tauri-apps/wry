---
"wry": patch
---

On macOS, the synthetic `mousedown`/`mouseup` of a mouse's back and forward buttons no longer throws `null is not an object (evaluating 'el.dispatchEvent')` where the pointer stands past the page's edge in CSS pixels, as under a page zoom: the point is converted by the view's `isFlipped` and `pageZoom`, and an event with no element under it goes to the document element.
