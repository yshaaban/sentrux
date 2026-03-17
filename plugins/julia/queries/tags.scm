; Julia tags.scm — functions, classes, modules, imports, calls, type references

; ── Definitions ──

; functions: function_definition → signature → call_expression → identifier
(function_definition
  (signature
    (call_expression
      (identifier) @name))) @definition.function

; macros: macro_definition → signature → call_expression → identifier
(macro_definition
  (signature
    (call_expression
      (identifier) @name))) @definition.function

; structs: struct_definition → type_head → identifier
(struct_definition
  (type_head
    (identifier) @name)) @definition.class

; abstract types: abstract_definition → type_head → identifier
(abstract_definition
  (type_head
    (identifier) @name)) @definition.class

; modules: module_definition has name field
(module_definition
  name: (identifier) @name) @definition.module

; ── Imports ──

(import_statement
  (identifier) @import.module) @import

; using: using_statement → (identifier) for module path
(using_statement
  (identifier) @import.module) @import

; ── Calls ──

; Direct function call: func(args)
(call_expression
  (identifier) @call.name) @call

; Method/qualified call: Module.func(args)
(call_expression
  (field_expression
    (identifier) @call.name)) @call

; ── Type references ──

; Type annotations: x::Type
(typed_expression
  (identifier) @reference.type)

; Parametric types: Vector{Int}
(parametrized_type_expression
  (identifier) @reference.type)
