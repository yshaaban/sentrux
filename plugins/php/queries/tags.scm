; Official tree-sitter-php tags.scm (v0.23.11)

(namespace_definition
  name: (namespace_name) @name) @definition.module

(interface_declaration
  name: (name) @name) @definition.interface

(trait_declaration
  name: (name) @name) @definition.interface

(class_declaration
  name: (name) @name) @definition.class

(class_interface_clause [(name) (qualified_name)] @name) @reference.implementation

(property_declaration
  (property_element (variable_name (name) @name))) @definition.field

(function_definition
  name: (name) @name) @definition.function

(method_declaration
  name: (name) @name) @definition.function

(object_creation_expression
  [
    (qualified_name (name) @name)
    (variable_name (name)) @name
  ]) @reference.class

(function_call_expression
  function: [
    (qualified_name (name) @name)
    (variable_name (name)) @name
  ]) @reference.call

(scoped_call_expression
  name: (name) @name) @reference.call

(member_call_expression
  name: (name) @name) @reference.call

; ---- Import appendix (custom) ----

; use App\Models\User;
(namespace_use_declaration
  (namespace_use_clause
    [(qualified_name) (name)] @import.module)) @import

; require_once 'file.php' / include 'file.php'
(include_expression
  (string (string_content) @import.module)) @import

; Anonymous functions (closures)
(anonymous_function_creation_expression
  (formal_parameters) @name) @definition.function

; Arrow functions
(arrow_function
  (formal_parameters) @name) @definition.function

; Trait use inside class: use SomeTrait;
(use_declaration
  (name) @import.module) @import
