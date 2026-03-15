; Protobuf tags.scm — verified against actual AST

; messages
(message
  (messageName
    (ident) @name)) @definition.class

; services
(service
  (serviceName
    (ident) @name)) @definition.class

; RPCs
(rpc
  (rpcName
    (ident) @name)) @definition.function

; imports: import → strLit
(import
  (strLit) @import.module) @import
