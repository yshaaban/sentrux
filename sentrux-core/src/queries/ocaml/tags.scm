; Official tree-sitter-ocaml tags.scm (v0.24.2) — simplified for our use

; Modules
(module_definition
  (module_binding (module_name) @name) @definition.module)

; Classes
(class_definition
  (class_binding (class_name) @name) @definition.class)

; Methods
(method_definition (method_name) @name) @definition.method

; Types
(type_definition
  (type_binding
    name: [
      (type_constructor) @name
      (type_constructor_path (type_constructor) @name)
    ]
  ) @definition.type)

; Functions
(value_definition
  [
    (let_binding pattern: (value_name) @name (parameter))
    (let_binding
      pattern: (value_name) @name
      body: [(fun_expression) (function_expression)]
    )
  ] @definition.function)

(external (value_name) @name) @definition.function

; Calls
(application_expression
  function: (value_path (value_name) @name)
) @reference.call

(method_invocation (method_name) @name) @reference.call
