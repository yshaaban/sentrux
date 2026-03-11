; Kotlin structural queries
; Note: Kotlin grammar has no named fields, uses child type matching

; Functions
(function_declaration
  (simple_identifier) @func.name) @func.def

; Classes
(class_declaration
  (type_identifier) @class.name) @class.def

; Objects (singleton)
(object_declaration
  (type_identifier) @class.name) @class.def

; Imports
(import_header
  (identifier) @import.module) @import

; Calls — direct
(call_expression
  (simple_identifier) @call.name) @call

; Calls — navigation  object.method()
(call_expression
  (navigation_expression
    (navigation_suffix
      (simple_identifier) @call.name))) @call
