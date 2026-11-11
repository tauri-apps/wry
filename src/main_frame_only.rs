// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Internal helper for platforms whose native API cannot restrict an initialization script to the
//! main frame (WebView2's `AddScriptToExecuteOnDocumentCreated` and Android's
//! `addDocumentStartJavaScript` inject into every frame). On these platforms scripts marked
//! `for_main_frame_only` are wrapped in a guard so they still only execute in the top-level frame.

use std::borrow::Cow;

use crate::InitializationScript;

impl InitializationScript {
  /// The source to hand to a native API that injects scripts into every frame.
  ///
  /// Scripts marked [`for_main_frame_only`](Self::for_main_frame_only) are guarded with
  /// [`guard_main_frame_only`], all others are returned unchanged.
  pub(crate) fn source_for_all_frames(&self) -> Cow<'_, str> {
    if self.for_main_frame_only {
      Cow::Owned(guard_main_frame_only(&self.script))
    } else {
      Cow::Borrowed(&self.script)
    }
  }
}

/// Wraps `script` so that it only executes in the top-level frame.
///
/// The body is placed in a block statement (`if (window === window.top) { ... }`) and not in a
/// function, so that top-level `var` declarations and sloppy-mode function declarations still
/// create globals in the main frame exactly like the unwrapped script would (bundlers commonly emit
/// `var Name = (() => { ... })();` for IIFE builds). In a subframe nothing in the body runs, so no
/// value or function source from the script becomes reachable there.
///
/// `window` and `window.top` are unforgeable and the script runs at document start before any page
/// script, so a subframe cannot make the guard pass.
///
/// A leading `"use strict";` / `'use strict';` directive is kept in front of the guard so that the
/// body stays in strict mode. Remaining differences to an unwrapped script: top-level `let`, `const`
/// and `class` declarations, and function declarations in strict mode, are scoped to the block
/// instead of the global scope; scripts that need to share such bindings with page scripts should
/// assign them to `window` explicitly.
pub(crate) fn guard_main_frame_only(script: &str) -> String {
  let (directive, body) = split_use_strict_directive(script);
  // The newline before the closing brace keeps a trailing `// comment` from swallowing it.
  format!("{directive}if (window === window.top) {{\n{body}\n}}")
}

/// Splits a leading `"use strict";` / `'use strict';` directive (with optional leading whitespace)
/// off `script`. Only the unambiguous form terminated by `;` is recognized: without the semicolon
/// the literal may be the start of a longer expression statement.
fn split_use_strict_directive(script: &str) -> (&str, &str) {
  let trimmed = script.trim_start();
  for literal in ["\"use strict\"", "'use strict'"] {
    if let Some(rest) = trimmed.strip_prefix(literal) {
      let after_ws = rest.trim_start_matches([' ', '\t']);
      if let Some(body) = after_ws.strip_prefix(';') {
        let directive_len = script.len() - body.len();
        return (&script[..directive_len], body);
      }
    }
  }
  ("", script)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn guards_body_with_top_frame_check() {
    assert_eq!(
      guard_main_frame_only("window.__KEY__ = 1;"),
      "if (window === window.top) {\nwindow.__KEY__ = 1;\n}"
    );
  }

  #[test]
  fn trailing_line_comment_does_not_swallow_closing_brace() {
    let guarded = guard_main_frame_only("var a = 1; // trailing");
    assert!(guarded.ends_with("// trailing\n}"));
  }

  #[test]
  fn keeps_use_strict_directive_in_front_of_guard() {
    assert_eq!(
      guard_main_frame_only("  'use strict';\nvar a = 1;"),
      "  'use strict';if (window === window.top) {\n\nvar a = 1;\n}"
    );
    assert_eq!(
      guard_main_frame_only("\"use strict\" ;var a = 1;"),
      "\"use strict\" ;if (window === window.top) {\nvar a = 1;\n}"
    );
  }

  #[test]
  fn does_not_hoist_ambiguous_or_non_leading_strings() {
    // Without `;` the literal might continue as an expression, e.g. `"use strict"\n(f)()`.
    assert_eq!(
      guard_main_frame_only("\"use strict\"\nvar a = 1;"),
      "if (window === window.top) {\n\"use strict\"\nvar a = 1;\n}"
    );
    assert_eq!(
      guard_main_frame_only("var a = 'use strict';"),
      "if (window === window.top) {\nvar a = 'use strict';\n}"
    );
  }

  #[test]
  fn only_main_frame_only_scripts_are_guarded() {
    let main_only = InitializationScript {
      script: "window.__KEY__ = 1;".into(),
      for_main_frame_only: true,
    };
    let all_frames = InitializationScript {
      script: "window.__ALL__ = 1;".into(),
      for_main_frame_only: false,
    };
    assert_eq!(
      main_only.source_for_all_frames(),
      "if (window === window.top) {\nwindow.__KEY__ = 1;\n}"
    );
    assert!(matches!(
      all_frames.source_for_all_frames(),
      Cow::Borrowed("window.__ALL__ = 1;")
    ));
  }
}
