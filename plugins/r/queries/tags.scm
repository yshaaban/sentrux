; R tags.scm — verified against actual AST

; function definitions: hello <- function(x) { x }
(binary_operator
    lhs: (identifier) @name
    operator: "<-"
    rhs: (function_definition)) @definition.function

(binary_operator
    lhs: (identifier) @name
    operator: "="
    rhs: (function_definition)) @definition.function

; calls
(call
    function: (identifier) @name) @reference.call

; imports: library(pkg) — argument is identifier, not string
(call
    function: (identifier) @_fn
    arguments: (arguments
        (argument
            value: (identifier) @import.module))
    (#any-of? @_fn "library" "require" "source")) @import

; imports with string: library("pkg")
(call
    function: (identifier) @_fn2
    arguments: (arguments
        (argument
            value: (string) @import.module))
    (#any-of? @_fn2 "library" "require" "source")) @import
