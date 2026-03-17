; Official tree-sitter-c tags.scm (v0.23.4)

(struct_specifier name: (type_identifier) @name body:(_)) @definition.class

(declaration type: (union_specifier name: (type_identifier) @name)) @definition.class

(function_declarator declarator: (identifier) @name) @definition.function

(type_definition declarator: (type_identifier) @name) @definition.type

(enum_specifier name: (type_identifier) @name) @definition.type

; ---- Custom additions for imports/calls ----

; Pointer function declarations (official misses these)
(function_definition
  declarator: (pointer_declarator
    declarator: (function_declarator
      declarator: (identifier) @func.name))) @func.def

; Includes
(preproc_include
  path: (string_literal) @import.module) @import

(preproc_include
  path: (system_lib_string) @import.module) @import

; Calls — direct
(call_expression
  function: (identifier) @call.name) @call

; Calls — member  ptr->func() or obj.func()
(call_expression
  function: (field_expression
    field: (field_identifier) @call.name)) @call

; Macro function definitions: #define FOO(x) ...
(preproc_function_def
  name: (identifier) @name) @definition.function

; Macro constant definitions: #define MAX 100
(preproc_def
  name: (identifier) @name) @definition.constant

; Type references
(type_identifier) @reference.type
