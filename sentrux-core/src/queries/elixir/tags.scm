; Official tree-sitter-elixir tags.scm (v0.3.5)

; modules and protocols
(call
  target: (identifier) @_kw
  (arguments (alias) @name)
  (#any-of? @_kw "defmodule" "defprotocol")) @definition.module

; functions/macros
(call
  target: (identifier) @_kw
  (arguments
    [
      (identifier) @name
      (call target: (identifier) @name)
      (binary_operator
        left: (call target: (identifier) @name)
        operator: "when")
    ])
  (#any-of? @_kw "def" "defp" "defdelegate" "defguard" "defguardp" "defmacro" "defmacrop" "defn" "defnp")) @definition.function

; ignore kernel/special-forms
(call
  target: (identifier) @_kw
  (#any-of? @_kw "def" "defp" "defdelegate" "defguard" "defguardp" "defmacro" "defmacrop" "defn" "defnp" "defmodule" "defprotocol" "defimpl" "defstruct" "defexception" "defoverridable" "alias" "case" "cond" "else" "for" "if" "import" "quote" "raise" "receive" "require" "reraise" "super" "throw" "try" "unless" "unquote" "unquote_splicing" "use" "with"))

; function calls
(call
  target: [
   (identifier) @name
   (dot
     right: (identifier) @name)
  ]) @reference.call

; pipe into function call
(binary_operator
  operator: "|>"
  right: (identifier) @name) @reference.call
