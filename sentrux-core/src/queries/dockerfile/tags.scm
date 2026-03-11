; Dockerfile structural queries (hand-written, no official tags.scm)

; FROM instructions (base image imports)
(from_instruction
  (image_spec
    name: (image_name) @import.module)) @import
