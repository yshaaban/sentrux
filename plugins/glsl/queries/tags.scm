; GLSL tags.scm

(function_definition
  declarator: (function_declarator
    declarator: (identifier) @name)) @definition.function

(struct_specifier
  name: (type_identifier) @name) @definition.class

; ---- Import appendix ----

(preproc_include
  path: (_) @import.module) @import
