; Dart tags.scm — verified against actual AST

; functions
(function_signature
  name: (identifier) @name) @definition.function

; classes
(class_definition
  name: (identifier) @name) @definition.class

; enum
(enum_declaration
  name: (identifier) @name) @definition.class

; mixin
(mixin_declaration
  name: (identifier) @name) @definition.class

; imports: import_or_export → library_import → import_specification → configurable_uri
(import_or_export
  (library_import
    (import_specification
      (configurable_uri) @import.module))) @import
