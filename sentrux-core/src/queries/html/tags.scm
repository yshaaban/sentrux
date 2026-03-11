; HTML structural queries

; Capture <script> and <style> inline blocks as class-like structures
(script_element) @definition.class
(style_element) @definition.class

; Capture <script src="..."> — the quoted value becomes the import module
(script_element
  (start_tag
    (attribute
      (attribute_name) @_attr
      (quoted_attribute_value) @import.module))) @import

; Capture self-closing tags with href/src (covers <link href="..." />)
(self_closing_tag
  (attribute
    (attribute_name) @_attr
    (quoted_attribute_value) @import.module)) @import

; Capture regular elements with href/src
(element
  (start_tag
    (attribute
      (attribute_name) @_attr
      (quoted_attribute_value) @import.module))) @import
