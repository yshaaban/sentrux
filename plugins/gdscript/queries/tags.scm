; GDScript tags.scm

(function_definition
  name: (name) @name) @definition.function

(class_name_statement
  name: (name) @name) @definition.class

(class_definition
  name: (name) @name) @definition.class

(call
  (identifier) @name) @reference.call

; ALL preload/load calls as imports (no predicate filter)
(call
  (identifier) @_fn
  (arguments
    (string) @import.module)) @import
