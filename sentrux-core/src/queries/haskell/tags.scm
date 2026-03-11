; Haskell structural queries (hand-written, no official tags.scm)

; Function bindings
(function
  name: (variable) @func.name) @func.def

; Type class declarations
(class
  name: (name) @class.name) @class.def

; Data type declarations
(data_type
  name: (name) @class.name) @class.def

; Newtype declarations
(newtype
  name: (name) @class.name) @class.def

; Import declarations
(import
  module: (module) @import.module) @import
