(require-builtin helix/core/editor as helix.)
(provide editor-focus)
;;@doc
;;
;;Get the current focus of the editor, as a `ViewId`.
;;
;;```scheme
;;(editor-focus) -> ViewId
;;```
;;        
(define (editor-focus)
    (helix.editor-focus *helix.cx*))

(provide editor-mode)
;;@doc
;;
;;Get the current mode of the editor
;;
;;```scheme
;;(editor-mode) -> Mode?
;;```
;;        
(define (editor-mode)
    (helix.editor-mode *helix.cx*))

(provide cx->themes)
;;@doc
;;DEPRECATED: Please use `themes->list`
(define (cx->themes)
    (helix.cx->themes *helix.cx*))

(provide editor-count)
;;@doc
;;Get the count
(define (editor-count)
    (helix.editor-count *helix.cx*))

(provide themes->list)
;;@doc
;;
;;Get the current themes as a list of strings.
;;
;;```scheme
;;(themes->list) -> (listof string?)
;;```
;;        
(define (themes->list)
    (helix.themes->list *helix.cx*))

(provide editor-all-documents)
;;@doc
;;
;;Get a list of all of the document ids that are currently open.
;;
;;```scheme
;;(editor-all-documents) -> (listof DocumentId?)
;;```
;;        
(define (editor-all-documents)
    (helix.editor-all-documents *helix.cx*))

(provide cx->cursor)
;;@doc
;;DEPRECATED: Please use `current-cursor`
(define (cx->cursor)
    (helix.cx->cursor *helix.cx*))

(provide current-cursor)
;;@doc
;;Gets the primary cursor position in screen coordinates,
;;or `#false` if the primary cursor is not visible on screen.
;;
;;```scheme
;;(current-cursor) -> (listof? (or Position? #false) CursorKind)
;;```
;;        
(define (current-cursor)
    (helix.current-cursor *helix.cx*))

(provide editor-focused-buffer-area)
;;@doc
;;
;;Get the `Rect` associated with the currently focused buffer.
;;
;;```scheme
;;(editor-focused-buffer-area) -> (or Rect? #false)
;;```
;;        
(define (editor-focused-buffer-area)
    (helix.editor-focused-buffer-area *helix.cx*))

(provide selected-register!)
;;@doc
;;Get currently selected register
(define (selected-register!)
    (helix.selected-register! *helix.cx*))

(provide Action/Load)
(define Action/Load helix.Action/Load)

(provide Action/Replace)
(define Action/Replace helix.Action/Replace)

(provide Action/HorizontalSplit)
(define Action/HorizontalSplit helix.Action/HorizontalSplit)

(provide Action/VerticalSplit)
(define Action/VerticalSplit helix.Action/VerticalSplit)

(provide string->editor-mode)
;;@doc
;;
;;Create an editor mode from a string, or false if it string was not one of
;;"normal", "insert", or "select"
;;
;;```scheme
;;(string->editor-mode "normal") -> (or Mode? #f)
;;```
;;        
(define (string->editor-mode arg)
    (helix.string->editor-mode *helix.cx* arg))

(provide editor->doc-id)
;;@doc
;;Get the document from a given view.
(define (editor->doc-id arg)
    (helix.editor->doc-id *helix.cx* arg))

(provide editor-switch!)
;;@doc
;;Open the document in a vertical split.
(define (editor-switch! arg)
    (helix.editor-switch! *helix.cx* arg))

(provide editor-set-focus!)
;;@doc
;;Set focus on the view.
(define (editor-set-focus! arg)
    (helix.editor-set-focus! *helix.cx* arg))

(provide editor-set-mode!)
;;@doc
;;Set the editor mode.
(define (editor-set-mode! arg)
    (helix.editor-set-mode! *helix.cx* arg))

(provide editor-doc-in-view?)
;;@doc
;;Check whether the current view contains a document.
(define (editor-doc-in-view? arg)
    (helix.editor-doc-in-view? *helix.cx* arg))

(provide set-scratch-buffer-name!)
;;@doc
;;Set the name of a scratch buffer.
(define (set-scratch-buffer-name! arg)
    (helix.set-scratch-buffer-name! *helix.cx* arg))

(provide set-buffer-uri!)
;;@doc
;;Set the URI of the buffer
(define (set-buffer-uri! arg)
    (helix.set-buffer-uri! *helix.cx* arg))

(provide editor-doc-exists?)
;;@doc
;;Check if a document exists.
(define (editor-doc-exists? arg)
    (helix.editor-doc-exists? *helix.cx* arg))

(provide editor-document-last-saved)
;;@doc
;;Check when a document was last saved (returns a `SystemTime`)
(define (editor-document-last-saved arg)
    (helix.editor-document-last-saved *helix.cx* arg))

(provide editor-document->language)
;;@doc
;;Get the language for the document
(define (editor-document->language arg)
    (helix.editor-document->language *helix.cx* arg))

(provide editor-document-dirty?)
;;@doc
;;Check if a document has unsaved changes
(define (editor-document-dirty? arg)
    (helix.editor-document-dirty? *helix.cx* arg))

(provide editor-document-reload)
;;@doc
;;Reload a document.
(define (editor-document-reload arg)
    (helix.editor-document-reload *helix.cx* arg))

(provide editor->text)
;;@doc
;;Get the document as a rope.
(define (editor->text arg)
    (helix.editor->text *helix.cx* arg))

(provide editor-document->path)
;;@doc
;;Get the path to a document.
(define (editor-document->path arg)
    (helix.editor-document->path *helix.cx* arg))

(provide register->value)
;;@doc
;;Get register value as a list of strings.
(define (register->value arg)
    (helix.register->value *helix.cx* arg))

(provide set-editor-clip-top!)
;;@doc
;;Set the editor clipping at the top.
(define (set-editor-clip-top! arg)
    (helix.set-editor-clip-top! *helix.cx* arg))

(provide set-editor-clip-right!)
;;@doc
;;Set the editor clipping at the right.
(define (set-editor-clip-right! arg)
    (helix.set-editor-clip-right! *helix.cx* arg))

(provide set-editor-clip-left!)
;;@doc
;;Set the editor clipping at the left.
(define (set-editor-clip-left! arg)
    (helix.set-editor-clip-left! *helix.cx* arg))

(provide set-editor-clip-bottom!)
;;@doc
;;Set the editor clipping at the bottom.
(define (set-editor-clip-bottom! arg)
    (helix.set-editor-clip-bottom! *helix.cx* arg))

(provide editor-switch-action!)
(define (editor-switch-action! arg1 arg2)
    (helix.editor-switch-action! *helix.cx* arg1 arg2))

(provide set-register!)
(define (set-register! arg1 arg2)
    (helix.set-register! *helix.cx* arg1 arg2))
