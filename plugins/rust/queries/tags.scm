; Official tree-sitter-rust tags.scm (v0.23.3)

; ADT definitions
(struct_item
    name: (type_identifier) @name) @definition.class

; Enums are algebraic data types (ADTs) — they provide polymorphic dispatch
; through pattern matching, equivalent to abstract classes/interfaces.
(enum_item
    name: (type_identifier) @name) @definition.adt

(union_item
    name: (type_identifier) @name) @definition.class

; type aliases
(type_item
    name: (type_identifier) @name) @definition.class

; method definitions
(declaration_list
    (function_item
        name: (identifier) @name) @definition.method)

; function definitions
(function_item
    name: (identifier) @name) @definition.function

; trait definitions
(trait_item
    name: (type_identifier) @name) @definition.interface

; module definitions
(mod_item
    name: (identifier) @name) @definition.module

; macro definitions
(macro_definition
    name: (identifier) @name) @definition.macro

; references
(call_expression
    function: (identifier) @name) @reference.call

(call_expression
    function: (field_expression
        field: (field_identifier) @name)) @reference.call

(macro_invocation
    macro: (identifier) @name) @reference.call

; implementations
(impl_item
    trait: (type_identifier) @name) @reference.implementation

(impl_item
    type: (type_identifier) @name
    !trait) @reference.implementation

; ---- Entry point: #[tokio::main] and similar attribute macros ----
(attribute_item) @entry

; ---- Import appendix (custom) ----

(use_declaration) @import

; mod declarations without body: `mod foo;` → import of sibling file
(mod_item
  !body) @import

; Type references — struct fields, function params, return types
(type_identifier) @reference.type

; Scoped path calls: crate::module::func() or std::thread::spawn()
; The full scoped_identifier is captured as @call.scoped_path for implicit import extraction.
(call_expression
  function: (scoped_identifier
    name: (identifier) @call.name) @call.scoped_path) @call
