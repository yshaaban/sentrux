; Solidity tags.scm — verified against actual AST

; functions
(function_definition
  name: (identifier) @name) @definition.function

; contracts
(contract_declaration
  name: (identifier) @name) @definition.class

; imports: import_directive → string field:source
(import_directive
  source: (string) @import.module) @import
