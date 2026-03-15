; Zig tags.scm — verified against actual AST

; functions: function_declaration → identifier field:name
(function_declaration
  name: (identifier) @name) @definition.function

; imports: variable_declaration with builtin @import
(variable_declaration
  (builtin_function
    (builtin_identifier) @_fn
    (arguments (string) @import.module)
    (#eq? @_fn "@import"))) @import
