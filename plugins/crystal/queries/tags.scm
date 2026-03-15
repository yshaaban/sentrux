; Crystal tags.scm (Ruby-like syntax)

(method_def
  name: (identifier) @name) @definition.function

(class_def
  name: (constant) @name) @definition.class

(module_def
  name: (constant) @name) @definition.module

(struct_def
  name: (constant) @name) @definition.class

(call
  method: (identifier) @name) @reference.call

; ---- Import appendix ----

(require
  (string) @import.module) @import
