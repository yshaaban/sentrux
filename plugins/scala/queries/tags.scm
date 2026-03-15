; Scala tags.scm — verified against actual AST

; functions
(function_definition
  name: (identifier) @name) @definition.function

; classes
(class_definition
  name: (identifier) @name) @definition.class

(object_definition
  name: (identifier) @name) @definition.class

(trait_definition
  name: (identifier) @name) @definition.class

; imports: import_declaration captures whole path via text
(import_declaration) @import
