; Bash tags.scm — verified against actual AST

; functions: function_definition → word field:name
(function_definition
  name: (word) @name) @definition.function

; calls: command → command_name → word
(command
  name: (command_name
    (word) @name)) @reference.call

; imports: source/. commands
; Match "source" keyword
(command
  name: (command_name
    (word) @_src)
  argument: (word) @import.module
  (#eq? @_src "source")) @import

; Match "." keyword  
(command
  name: (command_name
    (word) @_dot)
  argument: (word) @import.module
  (#eq? @_dot ".")) @import

; source with quoted string argument
(command
  name: (command_name
    (word) @_src2)
  argument: (raw_string) @import.module
  (#eq? @_src2 "source")) @import

(command
  name: (command_name
    (word) @_src3)
  argument: (string) @import.module
  (#eq? @_src3 "source")) @import
