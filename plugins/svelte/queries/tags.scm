; Svelte tags.scm
; Svelte files contain embedded script elements with JS/TS

(script_element) @definition.module

(element
  (start_tag
    (tag_name) @name)) @reference.call

; ---- Import appendix ----
; Svelte imports live inside <script> blocks parsed as raw_text.

(script_element) @import
