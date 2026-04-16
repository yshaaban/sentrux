use crate::core::types::FuncInfo;
use std::collections::HashSet;

/// Distribute calls to their containing functions.
/// Each call is assigned to the innermost function whose [sl, el] range
/// contains the call's source line. Calls outside any function go to
/// the file-level `co` (module-level code). Returns module-level calls.
pub(super) fn distribute_calls_to_functions(
    calls_raw: &[(String, u32)],
    functions: &mut [FuncInfo],
) -> Vec<String> {
    let mut sorted_indices: Vec<(u32, u32, usize)> = functions
        .iter()
        .enumerate()
        .map(|(index, function)| (function.sl, function.el, index))
        .collect();
    sorted_indices.sort_unstable_by_key(|&(start_line, _, _)| start_line);

    let mut function_calls: Vec<Vec<String>> = vec![Vec::new(); functions.len()];
    let mut module_calls: Vec<String> = Vec::new();
    let mut module_call_set: HashSet<String> = HashSet::new();

    for (call_name, line) in calls_raw {
        match find_containing_function(&sorted_indices, *line) {
            Some(index) => function_calls[index].push(call_name.clone()),
            None => {
                if module_call_set.insert(call_name.clone()) {
                    module_calls.push(call_name.clone());
                }
            }
        }
    }

    assign_deduped_calls(functions, &mut function_calls);
    module_calls
}

fn find_containing_function(sorted_indices: &[(u32, u32, usize)], line: u32) -> Option<usize> {
    let position = sorted_indices.partition_point(|&(start_line, _, _)| start_line <= line);
    if position == 0 {
        return None;
    }

    for index in (0..position).rev() {
        let (start_line, end_line, original_index) = sorted_indices[index];
        if line >= start_line && line <= end_line {
            return Some(original_index);
        }
    }

    None
}

fn assign_deduped_calls(functions: &mut [FuncInfo], function_calls: &mut [Vec<String>]) {
    for (index, function) in functions.iter_mut().enumerate() {
        if function_calls[index].is_empty() {
            continue;
        }

        let mut seen = HashSet::new();
        function_calls[index].retain(|call| seen.insert(call.clone()));
        function.co = Some(std::mem::take(&mut function_calls[index]));
    }
}
