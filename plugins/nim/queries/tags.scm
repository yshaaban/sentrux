; Nim tags.scm — verified against actual AST

; functions: routine → symbol → ident
(routine
  (symbol
    (ident) @name)) @definition.function

; types: typeDef → symbol → ident
(typeDef
  (symbol
    (ident) @name)) @definition.class

; imports: importStmt → expr (read text as module path)
(importStmt
  (expr) @import.module) @import
(fromStmt
  (expr) @import.module) @import
(includeStmt
  (expr) @import.module) @import
