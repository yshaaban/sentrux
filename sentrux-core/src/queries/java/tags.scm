; Official tree-sitter-java tags.scm (v0.23.5)

(class_declaration
  name: (identifier) @name) @definition.class

(method_declaration
  name: (identifier) @name) @definition.method

(method_invocation
  name: (identifier) @name
  arguments: (argument_list) @reference.call)

(interface_declaration
  name: (identifier) @name) @definition.interface

(type_list
  (type_identifier) @name) @reference.implementation

(object_creation_expression
  type: (type_identifier) @name) @reference.class

(superclass (type_identifier) @name) @reference.class

; ---- Import appendix + custom additions ----

(import_declaration) @import

; Constructors (not in official tags.scm)
(constructor_declaration
  name: (identifier) @func.name) @func.def
