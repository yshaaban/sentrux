; Swift tags.scm — TESTED against compiled tree-sitter-swift grammar
; Only patterns that compile without error. No guessed node names.

; ── Definitions ──

(class_declaration
  name: (type_identifier) @name) @definition.class

(protocol_declaration
  name: (type_identifier) @name) @definition.interface

(function_declaration
    name: (simple_identifier) @name) @definition.function

; ---- Entry point: @main attribute on struct ----
(attribute (user_type (type_identifier) @_n)) @entry

; Init declarations
(init_declaration) @func.def

; ── Imports ──

(import_declaration
  (identifier) @import.module) @import

; ── Calls ──

; Direct function call
(call_expression
  (simple_identifier) @call.name) @call

; Navigation call: object.method()
(call_expression
  (navigation_expression
    (navigation_suffix
      (simple_identifier) @call.name))) @call

; ── Type references ──
; Critical for Swift: same-target files don't import each other
; but DO reference each other's types

(user_type
  (type_identifier) @reference.type)

(inheritance_specifier
  (user_type
    (type_identifier) @reference.type))
