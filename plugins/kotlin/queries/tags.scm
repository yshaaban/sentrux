; Kotlin tags.scm — verified against actual AST

; functions: function_declaration → simple_identifier (child, no field)
(function_declaration
  (simple_identifier) @name) @definition.function

; classes: class_declaration → type_identifier (child)
(class_declaration
  (type_identifier) @name) @definition.class

; imports: import_header → identifier (child with dotted path)
(import_header
  (identifier) @import.module) @import
