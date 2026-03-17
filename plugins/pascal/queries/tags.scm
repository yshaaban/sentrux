; Object Pascal / Delphi — tree-sitter-pascal (Isopod/tree-sitter-pascal)
; Node types verified against grammar.js and node-types.json

; ── Function & procedure definitions ──
; declProc inherits name field from _declProc: identifier or genericDot

(defProc
  header: (declProc
    name: (identifier) @name)) @definition.function

(defProc
  header: (declProc
    name: (genericDot
      rhs: (identifier) @name))) @definition.method

(defProc
  header: (declProc
    name: (genericTpl
      entity: (identifier) @name))) @definition.function

; ── Type declarations (class, record, interface) ──
; declType has name + type fields; type can be declClass, declIntf, declHelper

(declType
  name: (identifier) @name
  type: (declClass)) @definition.class

(declType
  name: (identifier) @name
  type: (declIntf)) @definition.class

(declType
  name: (identifier) @name
  type: (declHelper)) @definition.class

; Generic class: TFoo<T> = class(...)
(declType
  name: (genericTpl
    entity: (identifier) @name)
  type: (declClass)) @definition.class

; ── Uses / import clauses ──
; declUses contains moduleName children (dot-separated identifiers)

(declUses) @import

; ── Function / procedure calls ──
; exprCall uses 'entity' field (from op.args helper), NOT 'function'

(exprCall
  entity: (identifier) @name) @reference.call

(exprCall
  entity: (exprDot
    rhs: (identifier) @name)) @reference.call

(exprCall
  entity: (exprTpl
    entity: (identifier) @name)) @reference.call

; ── Type references ──

(typeref) @reference.type
