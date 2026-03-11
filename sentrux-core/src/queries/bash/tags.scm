; Bash structural queries

; Functions
(function_definition
  name: (word) @func.name) @func.def

; Commands (calls)
(command
  name: (command_name) @call.name) @call
