; Julia tags.scm — verified against actual AST

; functions: function_definition → signature → call_expression → identifier
(function_definition
  (signature
    (call_expression
      (identifier) @name))) @definition.function

; macros
(macro_definition) @definition.function

; structs: struct_definition → type_head → identifier
(struct_definition
  (type_head
    (identifier) @name)) @definition.class

; abstract types
(abstract_definition) @definition.class

; modules
(module_definition) @definition.module

; imports
(import_statement
  (identifier) @import.module) @import
(using_statement) @import
