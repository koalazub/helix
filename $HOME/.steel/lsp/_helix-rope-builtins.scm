(define #%helix-rope-module (#%module "helix/core/text"))

(define (register-values module values)
  (map (lambda (ident) (#%module-add module (symbol->string ident) void)) values))
(register-values #%helix-rope-module '(rope-regex-find
rope-byte->line
rope->slice
rope-regex
rope-regex-find*
rope-line->char
rope->line
rope-char-ref
rope-starts-with?
rope-trim-start
string->rope
rope-regex-match?
rope->string
rope-regex-splitn
rope-insert-char
rope-char->byte
rope-regex-split
rope->byte-slice
rope-insert-string
rope-len-lines
RopeRegex?
rope-len-chars
rope-ends-with?
Rope?
rope-len-bytes
rope-char->line
rope-line->byte
))(#%module-add-doc #%helix-rope-module "rope-regex-find" "Find the first match in a given rope\n\n```scheme\n(rope-regex-find regex rope) -> Rope?\n```\n\n* regex: RopeRegex?\n* rope: Rope?\n            ")
(#%module-add-doc #%helix-rope-module "rope-byte->line" "Convert the given byte offset to a line offset for a given rope\n\n```scheme\n(rope-byte->line rope byte-index) -> int?\n```\n\n* rope : Rope?\n* byte-index : int?\n\n            ")
(#%module-add-doc #%helix-rope-module "rope->slice" "Take a slice from using character indices from the rope.\nReturns a new rope value.\n\n```scheme\n(rope->slice rope start end) -> Rope?\n```\n\n* rope : Rope?\n* start: (and positive? int?)\n* end: (and positive? int?)\n")
(#%module-add-doc #%helix-rope-module "rope-regex" "Build a new RopeRegex? with a string\n\n```scheme\n(rope-regex string) -> RopeRegex?\n```\n\n* string: string?\n            ")
(#%module-add-doc #%helix-rope-module "rope-regex-find*" "Find and return all matches in a given rope\n\n```scheme\n(rope-regex-find* regex rope) -> '(Rope?)\n```\n* regex: RopeRegex?\n* rope: Rope?\n            ")
(#%module-add-doc #%helix-rope-module "rope-line->char" "Convert the given line index to a character offset for a given rope\n\n```scheme\n(rope-line->char rope line-offset) -> int?\n```\n\n* rope : Rope?\n* line-offset: int?\n            ")
(#%module-add-doc #%helix-rope-module "rope->line" "Get the line at the given line index. Returns a rope.\n\n```scheme\n(rope->line rope index) -> Rope?\n\n```\n\n* rope : Rope?\n* index : (and positive? int?)\n")
(#%module-add-doc #%helix-rope-module "rope-char-ref" "Get the character at the given index")
(#%module-add-doc #%helix-rope-module "rope-starts-with?" "Check if the rope starts with a given pattern")
(#%module-add-doc #%helix-rope-module "rope-trim-start" "Remove the leading whitespace from the given rope")
(#%module-add-doc #%helix-rope-module "string->rope" "Converts a string into a rope.\n\n```scheme\n(string->rope value) -> Rope?\n```\n\n* value : string?\n            ")
(#%module-add-doc #%helix-rope-module "rope-regex-match?" "Returns if a regex is matching on a given rope\n\n```scheme\n(rope-regex->match? regex rope) -> bool?\n```\n\n* regex: RopeRegex?\n* rope: Rope?\n            ")
(#%module-add-doc #%helix-rope-module "rope->string" "Convert the given rope to a string")
(#%module-add-doc #%helix-rope-module "rope-regex-splitn" "Split n times on the match in a given rope, return the rest\n\n```scheme\n(rope-regex-splitn regex rope n) -> '(Rope?)\n```\n\n* regex: RopeRegex?\n* rope: Rope?\n* n: (and positive? int?)\n")
(#%module-add-doc #%helix-rope-module "rope-insert-char" "Insert a character at the given index")
(#%module-add-doc #%helix-rope-module "rope-char->byte" "Convert the byte offset into a character offset for a given rope")
(#%module-add-doc #%helix-rope-module "rope-regex-split" "Split on the match in a given rope\n\n```scheme\n(rope-regex-split regex rope) -> '(Rope?)\n```\n\n* regex: RopeRegex?\n* rope: Rope?\n")
(#%module-add-doc #%helix-rope-module "rope->byte-slice" "Take a slice of this rope using byte offsets\n\n```scheme\n(rope->byte-slice rope start end) -> Rope?\n```\n\n* rope: Rope?\n* start: (and positive? int?)\n* end: (and positive? int?)\n")
(#%module-add-doc #%helix-rope-module "rope-insert-string" "Insert a string at the given index into the rope")
(#%module-add-doc #%helix-rope-module "rope-len-lines" "Get the number of lines in the rope")
(#%module-add-doc #%helix-rope-module "RopeRegex?" "Check if the given value is a rope regex")
(#%module-add-doc #%helix-rope-module "rope-len-chars" "Get the length of the rope in characters")
(#%module-add-doc #%helix-rope-module "rope-ends-with?" "Check if the rope ends with a given pattern")
(#%module-add-doc #%helix-rope-module "Rope?" "Check if the given value is a rope")
(#%module-add-doc #%helix-rope-module "rope-len-bytes" "Get the length of the rope in bytes")
(#%module-add-doc #%helix-rope-module "rope-char->line" "Convert the given character offset to a line offset for a given rope\n\n```scheme\n(rope-char->line rope char-index) -> int?\n```\n\n* rope : Rope?\n* char-index : int?\n\n            ")
(#%module-add-doc #%helix-rope-module "rope-line->byte" "Convert the given line index to a byte offset for a given rope\n\n```scheme\n(rope-line->byte rope line-offset) -> int?\n```\n\n* rope : Rope?\n* line-offset: int?\n            ")
