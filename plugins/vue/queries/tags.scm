; Vue tags.scm
; Vue SFC files contain template, script, and style sections

(script_element) @definition.module

(element
  (start_tag
    (tag_name) @name)) @reference.call

; ---- Import appendix ----
; Vue imports live inside <script> blocks parsed as raw_text.

(script_element) @import
