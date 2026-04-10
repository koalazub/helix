(define #%helix-themes-module (#%module "helix/core/themes"))

(define (register-values module values)
  (map (lambda (ident) (#%module-add module (symbol->string ident) void)) values))
(register-values #%helix-themes-module '(hashmap->theme
add-theme!
theme-style
theme-set-style!
string->color
))