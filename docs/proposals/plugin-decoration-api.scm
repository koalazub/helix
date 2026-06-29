;; Sketch only, not wired up. This illustrates the shape of a plugin-facing
;; decoration API discussed in the pull request description. The names and arguments are
;; placeholders for discussion, not a committed interface.

;; Attach a set of decorations to a view's buffer. Returns a handle the plugin
;; keeps in order to update or clear them later.
;;
;;   (set-decorations! view-id decorations) -> handle?
;;
;; where each decoration is one of:
;;
;;   (inline       char-idx text style)   ; text drawn inline at a position
;;   (virtual-rows line-idx rows)         ; rows reserved in the text flow
;;   (style-overlay start end style)      ; restyle or conceal a buffer range

;; Replace the decorations behind a handle without clearing first.
;;
;;   (update-decorations! handle decorations)

;; Remove them.
;;
;;   (clear-decorations! handle)

;; The engine remaps decoration positions across edits, the same way it already
;; does for inlay hints and diagnostics, so a handle stays valid as the buffer
;; changes. Image payloads, which rely on terminal graphics, are left out of this
;; sketch on purpose and would be a separate discussion.
