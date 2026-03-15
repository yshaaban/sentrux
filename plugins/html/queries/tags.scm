; HTML tags.scm — external resources as imports
; Captures href/src attribute values from link, script, and img tags

(attribute
  (attribute_name) @_attr
  (quoted_attribute_value
    (attribute_value) @import.module)
  (#any-of? @_attr "href" "src")) @import
