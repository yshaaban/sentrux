; Objective-C tags.scm — verified against actual AST

; functions: function_definition → function_declarator → identifier
(function_definition
  declarator: (function_declarator
    declarator: (identifier) @name)) @definition.function

; methods (captured without name extraction — ObjC method names are complex)
(method_declaration) @definition.function

; classes: class_interface → identifier (direct child, no field name)
(class_interface
  (identifier) @name) @definition.class

; protocols
(protocol_declaration
  (identifier) @name) @definition.interface

; imports: preproc_include with field:path
(preproc_include
  path: (_) @import.module) @import
