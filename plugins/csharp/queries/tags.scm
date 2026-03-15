; Official tree-sitter-c-sharp tags.scm (v0.23.1)

(class_declaration name: (identifier) @name) @definition.class

(class_declaration (base_list (_) @name)) @reference.class

(interface_declaration name: (identifier) @name) @definition.interface

(interface_declaration (base_list (_) @name)) @reference.interface

(method_declaration name: (identifier) @name) @definition.method

(object_creation_expression type: (identifier) @name) @reference.class

(type_parameter_constraints_clause (identifier) @name) @reference.class

(type_parameter_constraint (type type: (identifier) @name)) @reference.class

(variable_declaration type: (identifier) @name) @reference.class

(invocation_expression function: (member_access_expression name: (identifier) @name)) @reference.send

(namespace_declaration name: (identifier) @name) @definition.module

; ---- Custom additions for structs/enums/constructors/imports/calls ----

; Structs
(struct_declaration
  name: (identifier) @class.name) @class.def

; Enums
(enum_declaration
  name: (identifier) @class.name) @class.def

; Constructor
(constructor_declaration
  name: (identifier) @func.name) @func.def

; Using directives
(using_directive) @import

; Calls — direct
(invocation_expression
  function: (identifier) @call.name) @call
