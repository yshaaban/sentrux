; Dart tags.scm — functions, classes, imports, calls, type references

; ── Definitions ──

(function_signature
  name: (identifier) @name) @definition.function

(class_definition
  name: (identifier) @name) @definition.class

(enum_declaration
  name: (identifier) @name) @definition.class

; mixin_declaration has identifier as positional child, not named field
(mixin_declaration
  (identifier) @name) @definition.class

; ── Imports ──

(import_or_export
  (library_import
    (import_specification
      (configurable_uri) @import.module))) @import

; ── Calls ──

; Function call: identifier(args)
(call_expression
  function: (identifier) @call.name) @call

; Method call: obj.method(args)
(call_expression
  function: (selector
    (identifier) @call.name)) @call

; Constructor: new ClassName(args)
(new_expression
  (type_identifier) @call.name) @call

; ── Method definitions inside classes ──

(method_signature
  name: (identifier) @name) @definition.method

; ── Extension declarations ──

(extension_declaration
  name: (identifier) @name) @definition.class

; ── Additional imports ──

; part 'file.dart'
(part_directive
  (uri) @import.module) @import

; part of 'library'
(part_of_directive
  (uri) @import.module) @import

; export 'file.dart'
(export_directive
  (configurable_uri) @import.module) @import

; ── Type references ──

; Type annotations: Type x, List<Type>, Map<K, V>
(type_identifier) @reference.type
