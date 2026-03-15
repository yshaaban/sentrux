; Haskell tags.scm — verified against actual AST (data/class/newtype/function)

; functions
(declarations
  (function
    name: (variable) @name) @definition.function)

; data types: data Color = Red | Green | Blue
(declarations
  (data_type
    name: (_) @name) @definition.class)

; newtypes: newtype Name = Name String
(declarations
  (newtype
    name: (_) @name) @definition.class)

; type classes: class Printable a where ...
(declarations
  (class
    name: (_) @name) @definition.class)

; imports
(imports
  (import
    module: (module) @import.module) @import)
