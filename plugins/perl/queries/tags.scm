; Perl tags.scm — verified against actual AST

; functions: subroutine_declaration_statement → bareword field:name
(subroutine_declaration_statement
  name: (bareword) @name) @definition.function

; packages (as classes)
(package_statement
  name: (package) @name) @definition.class

; imports: use_statement → package field:module
(use_statement
  module: (package) @import.module) @import

; require
(require_expression) @import
