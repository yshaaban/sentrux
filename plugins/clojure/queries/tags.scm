; Clojure tags.scm
; Clojure uses list forms: (defn name ...), (def name ...), (ns name ...)

(list_lit
  value: (sym_lit) @name
  (#match? @name "^(defn|defn-|defmacro|defmethod|defmulti)$")
  value: (sym_lit) @func.name) @definition.function

(list_lit
  value: (sym_lit) @name
  (#match? @name "^(defprotocol|defrecord|deftype|definterface)$")
  value: (sym_lit) @class.name) @definition.class

; ---- Import appendix ----
; UNTESTED: @import.module captures are best-effort without grammar validation

; ns form: (ns my.namespace (:require ...))
; The second sym_lit child after "ns" is the namespace (module path)
(list_lit
  value: (sym_lit) @_ns_kw
  (#eq? @_ns_kw "ns")
  value: (sym_lit) @import.module) @import

; require form: (require '[clojure.string :as str])
; Fallback: capture the whole require/use/import form
(list_lit
  value: (sym_lit) @_req_kw
  (#match? @_req_kw "^(require|use|import)$")) @import
