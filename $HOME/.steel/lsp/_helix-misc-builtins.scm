(define #%helix-misc-module (#%module "helix/core/misc"))

(define (register-values module values)
  (map (lambda (ident) (#%module-add module (symbol->string ident) void)) values))
(register-values #%helix-misc-module '(push-component!
cursor-position
lsp-client-offset-encoding
set-error!
pop-last-component-by-name!
on-key-callback
lsp-reply-ok
hx.cx->pos
acquire-context-lock
lsp-client-name
send-lsp-notification
lsp-client-initialized?
mode-switch-old
mode-switch-new
enqueue-thread-local-callback-with-delay
hx.custom-insert-newline
set-status!
send-lsp-command
insert-newline-hook
await-callback
enqueue-thread-local-callback
pop-last-component!
remove-inlay-hint-by-id
get-active-lsp-clients
add-inlay-hint
helix-await-callback
set-warning!
trigger-on-key-callback
remove-inlay-hint
))