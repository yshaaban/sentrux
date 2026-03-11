; SCSS structural queries

; Mixins  @mixin name { ... }
(mixin_statement
  name: (identifier) @func.name) @func.def

; Functions  @function name() { ... }
(function_statement
  name: (identifier) @func.name) @func.def

; Imports
(import_statement) @import

; Includes  @include name;
(include_statement
  (identifier) @call.name) @call
