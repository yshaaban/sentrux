; Official tree-sitter-cpp tags.scm (v0.23.4)

(struct_specifier name: (type_identifier) @name body:(_)) @definition.class

(declaration type: (union_specifier name: (type_identifier) @name)) @definition.class

(function_declarator declarator: (identifier) @name) @definition.function

(function_declarator declarator: (field_identifier) @name) @definition.function

(function_declarator declarator: (qualified_identifier scope: (namespace_identifier) @local.scope name: (identifier) @name)) @definition.method

(type_definition declarator: (type_identifier) @name) @definition.type

(enum_specifier name: (type_identifier) @name) @definition.type

(class_specifier name: (type_identifier) @name) @definition.class

; ---- Custom additions for imports/calls ----

; Pointer function declarations
(function_definition
  declarator: (pointer_declarator
    declarator: (function_declarator
      declarator: (identifier) @func.name))) @func.def

; Reference function declarations
(function_definition
  declarator: (reference_declarator
    (function_declarator
      declarator: (identifier) @func.name))) @func.def

; Includes
(preproc_include
  path: (string_literal) @import.module) @import

(preproc_include
  path: (system_lib_string) @import.module) @import

; Calls — direct
(call_expression
  function: (identifier) @call.name) @call

; Calls — member
(call_expression
  function: (field_expression
    field: (field_identifier) @call.name)) @call

; Calls — qualified  Foo::bar() or std::cout
(call_expression
  function: (qualified_identifier
    name: (identifier) @call.name)) @call

; Calls — new constructor  new Foo()
(new_expression
  type: (type_identifier) @call.name) @call
