; PowerShell tags.scm — verified against actual AST

; functions: function_statement → function_name
(function_statement
  (function_name) @name) @definition.function

; classes: class_statement → simple_name
(class_statement
  (simple_name) @name) @definition.class

; enums
(enum_statement
  (simple_name) @name) @definition.class
