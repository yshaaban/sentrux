; Scala structural queries

; Function definitions
(function_definition
  name: (identifier) @func.name) @func.def

; Class definitions
(class_definition
  name: (identifier) @class.name) @class.def

; Object definitions (singleton)
(object_definition
  name: (identifier) @class.name) @class.def

; Trait definitions
(trait_definition
  name: (identifier) @class.name) @class.def

; Imports
(import_declaration) @import

; Calls — direct
(call_expression
  function: (identifier) @call.name) @call

; Calls — field access  obj.method()
(call_expression
  function: (field_expression
    field: (identifier) @call.name)) @call
