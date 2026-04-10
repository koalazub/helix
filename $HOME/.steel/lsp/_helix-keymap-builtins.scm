(define #%helix-keymap-module (#%module "helix/core/keymaps"))

(define (register-values module values)
  (map (lambda (ident) (#%module-add module (symbol->string ident) void)) values))
(register-values #%helix-keymap-module '(#%add-extension-or-labeled-keymap
helix-empty-keymap
keymap-update-documentation!
helix-string->keymap
helix-deep-copy-keymap
helix-merge-keybindings
#%add-reverse-mapping
query-keymap
helix-default-keymap
keymap?
))