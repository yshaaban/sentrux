; Zig structural queries (hand-written, no official tags.scm)

; Function declarations
(function_declaration
  name: (identifier) @func.name) @func.def

; Test declarations
(test_declaration
  (identifier) @func.name) @func.def

(test_declaration
  (string) @func.name) @func.def
