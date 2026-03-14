; Based on official tree-sitter-swift tags.scm (v0.7.1)
; Modified: removed class_body method patterns (they assign class-level line numbers)
; Using standalone function/init patterns which give correct per-function line numbers.

(class_declaration
  name: (type_identifier) @name) @definition.class

(protocol_declaration
  name: (type_identifier) @name) @definition.interface

(function_declaration
    name: (simple_identifier) @name) @definition.function

; ---- Entry point: @main attribute on struct ----
(attribute (user_type (type_identifier) @_n)) @entry

; ---- Import appendix + calls (custom) ----

; Init declarations
(init_declaration) @func.def

; import Foundation / import UIKit.NSView
(import_declaration
  (identifier) @import.module) @import

; Calls — direct function call
(call_expression
  (simple_identifier) @call.name) @call

; Calls — navigation  object.method()
(call_expression
  (navigation_expression
    (navigation_suffix
      (simple_identifier) @call.name))) @call
