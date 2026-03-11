; Official tree-sitter-python tags.scm (v0.23.6)

(module (expression_statement (assignment left: (identifier) @name) @definition.constant))

(class_definition
  name: (identifier) @name) @definition.class

(function_definition
  name: (identifier) @name) @definition.function

(call
  function: [
      (identifier) @name
      (attribute
        attribute: (identifier) @name)
  ]) @reference.call

; ---- Entry point: if __name__ == "__main__" ----
(if_statement
  condition: (comparison_operator) @entry)

; ---- Import appendix (custom) ----

(import_from_statement
  module_name: (dotted_name) @import.module) @import

(import_statement) @import
