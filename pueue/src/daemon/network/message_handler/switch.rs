use std::collections::HashSet;

use pueue_lib::{Settings, TaskStatus, failure_msg, message::*};

use super::ok_or_failure_message;
use crate::{daemon::internal_state::SharedState, ok_or_save_state_failure};

/// Invoked when calling `pueue switch`.
/// Switch the positions of tasks in the upcoming queue by swapping two lists pairwise.
/// We have to ensure that those tasks are either `Queued` or `Stashed`
pub fn switch(settings: &Settings, state: &SharedState, message: SwitchRequest) -> Response {
    let mut state = state.lock().unwrap();

    let task_ids_1 = message.task_ids_1;
    let task_ids_2 = message.task_ids_2;

    // Validate that both lists have the same length
    if task_ids_1.len() != task_ids_2.len() {
        return failure_msg!(
            "Both task ID lists must have the same length. Got {} and {} tasks.",
            task_ids_1.len(),
            task_ids_2.len()
        );
    }

    if task_ids_1.is_empty() {
        return failure_msg!("Task ID lists cannot be empty.");
    }

    // Collect all unique task IDs that need to be validated
    let mut all_ids: HashSet<usize> = HashSet::new();
    all_ids.extend(&task_ids_1);
    all_ids.extend(&task_ids_2);

    // Verify all tasks exist and are either queued or stashed
    let filtered_tasks = state.filter_tasks(
        |task| {
            matches!(
                task.status,
                TaskStatus::Queued { .. } | TaskStatus::Stashed { .. }
            )
        },
        Some(all_ids.into_iter().collect()),
    );
    if !filtered_tasks.non_matching_ids.is_empty() {
        return failure_msg!("All tasks must be either queued or stashed.");
    }

    // Perform pairwise swapping
    for (id1, id2) in task_ids_1.iter().zip(task_ids_2.iter()) {
        // Skip if trying to swap a task with itself (it's a no-op)
        if id1 == id2 {
            continue;
        }

        // Get the tasks
        let mut task1 = state.tasks_mut().remove(id1).unwrap();
        let mut task2 = state.tasks_mut().remove(id2).unwrap();

        // Switch task ids
        let old_id1 = task1.id;
        let old_id2 = task2.id;
        task1.id = old_id2;
        task2.id = old_id1;

        // Put tasks back with swapped IDs
        state.tasks_mut().insert(task1.id, task1);
        state.tasks_mut().insert(task2.id, task2);

        // Update dependencies in all other tasks
        for (_, task) in state.tasks_mut().iter_mut() {
            // If the task depends on both, we can just keep it as it is.
            if task.dependencies.contains(&old_id1) && task.dependencies.contains(&old_id2) {
                continue;
            }

            // If one of the ids is in the task's dependency list, replace it with the other one.
            if let Some(old_id) = task.dependencies.iter_mut().find(|id| **id == old_id1) {
                *old_id = old_id2;
                task.dependencies.sort_unstable();
            } else if let Some(old_id) = task.dependencies.iter_mut().find(|id| **id == old_id2) {
                *old_id = old_id1;
                task.dependencies.sort_unstable();
            }
        }
    }

    ok_or_save_state_failure!(state.save(settings));

    let swap_count = task_ids_1.len();
    if swap_count == 1 {
        create_success_response("Tasks have been switched")
    } else {
        create_success_response(format!("{} pairs of tasks have been switched", swap_count))
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    use super::{super::fixtures::*, *};

    fn get_message(task_ids_1: Vec<usize>, task_ids_2: Vec<usize>) -> SwitchRequest {
        SwitchRequest {
            task_ids_1,
            task_ids_2,
        }
    }

    fn get_test_state() -> (SharedState, Settings, TempDir) {
        let (state, settings, tempdir) = get_state();

        {
            let mut state = state.lock().unwrap();
            let task = get_stub_task("0", StubStatus::Queued);
            state.add_task(task);

            let task = get_stub_task("1", StubStatus::Stashed { enqueue_at: None });
            state.add_task(task);

            let task = get_stub_task("2", StubStatus::Queued);
            state.add_task(task);

            let task = get_stub_task("3", StubStatus::Stashed { enqueue_at: None });
            state.add_task(task);

            let mut task = get_stub_task("4", StubStatus::Queued);
            task.dependencies = vec![0, 3];
            state.add_task(task);

            let mut task = get_stub_task("5", StubStatus::Stashed { enqueue_at: None });
            task.dependencies = vec![1];
            state.add_task(task);

            let mut task = get_stub_task("6", StubStatus::Queued);
            task.dependencies = vec![2, 3];
            state.add_task(task);
        }

        (state, settings, tempdir)
    }

    #[test]
    /// A normal switch between two id's works perfectly fine.
    fn switch_normal() {
        let (state, settings, _tempdir) = get_test_state();

        let response = switch(&settings, &state, get_message(vec![1], vec![2]));

        // Response is correct
        assert!(matches!(response, Response::Success(_)));
        if let Response::Success(text) = response {
            assert_eq!(text, "Tasks have been switched");
        };

        let state = state.lock().unwrap();
        assert_eq!(state.tasks().get(&1).unwrap().command, "2");
        assert_eq!(state.tasks().get(&2).unwrap().command, "1");
    }

    #[test]
    /// Test batch switching of multiple task pairs.
    fn switch_batch() {
        let (state, settings, _tempdir) = get_test_state();

        let response = switch(&settings, &state, get_message(vec![0, 1], vec![2, 3]));

        // Response is correct
        assert!(matches!(response, Response::Success(_)));
        if let Response::Success(text) = response {
            assert_eq!(text, "2 pairs of tasks have been switched");
        };

        let state = state.lock().unwrap();
        // Check that tasks were swapped correctly
        assert_eq!(state.tasks().get(&0).unwrap().command, "2");
        assert_eq!(state.tasks().get(&2).unwrap().command, "0");
        assert_eq!(state.tasks().get(&1).unwrap().command, "3");
        assert_eq!(state.tasks().get(&3).unwrap().command, "1");
    }

    #[test]
    /// Test that mismatched list lengths are rejected.
    fn switch_mismatched_lengths() {
        let (state, settings, _tempdir) = get_test_state();

        let response = switch(&settings, &state, get_message(vec![1, 2], vec![3]));

        // Response is correct
        assert!(matches!(response, Response::Failure(_)));
        if let Response::Failure(text) = response {
            assert_eq!(
                text,
                "Both task ID lists must have the same length. Got 2 and 1 tasks."
            );
        };
    }

    #[test]
    /// Switching a task with itself is allowed (it's a no-op).
    fn switch_task_with_itself_allowed() {
        let (state, settings, _tempdir) = get_test_state();

        let response = switch(&settings, &state, get_message(vec![1], vec![1]));

        // Should succeed
        assert!(matches!(response, Response::Success(_)));

        // Task should remain unchanged
        let state = state.lock().unwrap();
        assert_eq!(state.tasks().get(&1).unwrap().command, "1");
    }

    #[test]
    /// Duplicate IDs are allowed (redundant swaps are ok).
    fn switch_with_duplicates_allowed() {
        let (state, settings, _tempdir) = get_test_state();

        // Switch 0↔1 twice (via duplicates)
        let response = switch(&settings, &state, get_message(vec![0, 0], vec![1, 1]));

        // Should succeed
        assert!(matches!(response, Response::Success(_)));

        // After swapping twice, tasks should be back to original positions
        let state = state.lock().unwrap();
        assert_eq!(state.tasks().get(&0).unwrap().command, "0");
        assert_eq!(state.tasks().get(&1).unwrap().command, "1");
    }

    #[test]
    /// If any task that is specified as dependency get's switched,
    /// all dependants need to be updated.
    fn switch_task_with_dependant() {
        let (state, settings, _tempdir) = get_test_state();

        switch(&settings, &state, get_message(vec![0], vec![3]));

        let state = state.lock().unwrap();
        assert_eq!(state.tasks().get(&4).unwrap().dependencies, vec![0, 3]);
    }

    #[test]
    /// A task with two dependencies shouldn't experience any change, if those two dependencies
    /// switched places.
    fn switch_double_dependency() {
        let (state, settings, _tempdir) = get_test_state();

        switch(&settings, &state, get_message(vec![1], vec![2]));

        let state = state.lock().unwrap();
        assert_eq!(state.tasks().get(&5).unwrap().dependencies, vec![2]);
        assert_eq!(state.tasks().get(&6).unwrap().dependencies, vec![1, 3]);
    }

    #[test]
    /// You can only switch tasks that are either stashed or queued.
    /// Everything else should result in an error message.
    fn switch_invalid() {
        let (state, settings, _tempdir) = get_state();

        let combinations: Vec<(Vec<usize>, Vec<usize>)> = vec![
            (vec![0], vec![1]), // Queued + Done
            (vec![0], vec![3]), // Queued + Stashed
            (vec![0], vec![4]), // Queued + Running
            (vec![0], vec![5]), // Queued + Paused
            (vec![2], vec![1]), // Stashed + Done
            (vec![2], vec![3]), // Stashed + Stashed
            (vec![2], vec![4]), // Stashed + Running
            (vec![2], vec![5]), // Stashed + Paused
        ];

        for ids in combinations {
            let response = switch(&settings, &state, get_message(ids.0, ids.1));

            // Assert, that we get a failure with the correct text.
            assert!(matches!(response, Response::Failure(_)));
            if let Response::Failure(text) = response {
                assert_eq!(text, "All tasks must be either queued or stashed.");
            };
        }
    }
}
