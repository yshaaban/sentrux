export const ExhaustivenessSiteKind = {
    Switch: "switch",
    Record: "record",
    Satisfies: "satisfies",
    IfElse: "if_else",
};
export const ExhaustivenessProofKind = {
    Switch: "switch",
    AssertNever: "assertNever",
    Record: "Record",
    Satisfies: "satisfies",
    IfElse: "if_else",
};
export const ExhaustivenessFallbackKind = {
    None: "none",
    Null: "null",
    Undefined: "undefined",
    GenericString: "generic_string",
    IdentityTransform: "identity_transform",
    EmptyArray: "empty_array",
    EmptyObject: "empty_object",
    AssertThrow: "assert_throw",
    Other: "other",
};
export const ExhaustivenessSiteSemanticRole = {
    Label: "label",
    Target: "target",
    Status: "status",
    Render: "render",
    Handler: "handler",
    Policy: "policy",
    Serialization: "serialization",
    Transform: "transform",
    Unknown: "unknown",
};
export const TransitionKind = {
    RecordEntry: "record_entry",
    SwitchCase: "switch_case",
    IfBranch: "if_branch",
    IfElse: "if_else",
};
