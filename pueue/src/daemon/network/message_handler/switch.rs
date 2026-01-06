use pueue_lib::{Settings, TaskStatus, failure_msg, message::*};
use std::collections::HashSet;

use super::ok_or_failure_message;
use crate::{daemon::internal_state::SharedState, ok_or_save_state_failure};

/// Invoked when calling `pueue switch`.
/// Switch the position of two tasks in the upcoming queue.
/// We have to ensure that those tasks are either `Queued` or `Stashed`
pub fn switch(settings: &Settings, state: &SharedState, message: SwitchRequest) -> Response {
    let mut state = state.lock().unwrap();

    let task_ids = [message.task_id_1, message.task_id_2];
    let filtered_tasks = state.filter_tasks(
        |task| {
            matches!(
                task.status,
                TaskStatus::Queued { .. } | TaskStatus::Stashed { .. }
            )
        },
        Some(task_ids.to_vec()),
    );
    if !filtered_tasks.non_matching_ids.is_empty() {
        return failure_msg!("Tasks have to be either queued or stashed.");
    }
    if task_ids[0] == task_ids[1] {
        return failure_msg!("You cannot switch a task with itself.");
    }

    // Get the tasks. Expect them to be there, since we found no mismatch
    let mut first_task = state.tasks_mut().remove(&task_ids[0]).unwrap();
    let mut second_task = state.tasks_mut().remove(&task_ids[1]).unwrap();

    // Switch task ids
    let first_id = first_task.id;
    let second_id = second_task.id;
    first_task.id = second_id;
    second_task.id = first_id;

    // Collect unique dependencies and dependents (excluding common ones)
    let first_deps_set: HashSet<_> = first_task.dependencies.iter().copied().collect();
    let second_deps_set: HashSet<_> = second_task.dependencies.iter().copied().collect();
    let unique_dependencies: Vec<usize> = first_deps_set
        .symmetric_difference(&second_deps_set)
        .copied()
        .collect();

    let first_dependents_set: HashSet<_> = first_task.dependents.iter().copied().collect();
    let second_dependents_set: HashSet<_> = second_task.dependents.iter().copied().collect();
    let unique_dependents: Vec<usize> = first_dependents_set
        .symmetric_difference(&second_dependents_set)
        .copied()
        .collect();

    // Put tasks back in again
    state.tasks_mut().insert(first_task.id, first_task);
    state.tasks_mut().insert(second_task.id, second_task);

    // Update dependents lists: swap first_id <-> second_id
    for &dep_id in &unique_dependencies {
        if let Some(dep_task) = state.tasks_mut().get_mut(&dep_id) {
            for id in dep_task.dependents.iter_mut() {
                if *id == first_id {
                    *id = second_id;
                } else if *id == second_id {
                    *id = first_id;
                }
            }
            dep_task.dependents.sort_unstable();
        }
    }

    // Update dependencies lists: swap first_id <-> second_id
    for &dependent_id in &unique_dependents {
        if let Some(dependent_task) = state.tasks_mut().get_mut(&dependent_id) {
            for id in dependent_task.dependencies.iter_mut() {
                if *id == first_id {
                    *id = second_id;
                } else if *id == second_id {
                    *id = first_id;
                }
            }
            dependent_task.dependencies.sort_unstable();
        }
    }

    ok_or_save_state_failure!(state.save(settings));
    create_success_response("Tasks have been switched")
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    use super::{super::fixtures::*, *};

    fn get_message(task_id_1: usize, task_id_2: usize) -> SwitchRequest {
        SwitchRequest {
            task_id_1,
            task_id_2,
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

            // Manually update dependents lists to match dependencies
            // This mimics the behavior of the add_task message handler
            state.tasks_mut().get_mut(&0).unwrap().dependents = vec![4];
            state.tasks_mut().get_mut(&1).unwrap().dependents = vec![5];
            state.tasks_mut().get_mut(&2).unwrap().dependents = vec![6];
            state.tasks_mut().get_mut(&3).unwrap().dependents = vec![4, 6];
        }

        (state, settings, tempdir)
    }

    #[test]
    /// A normal switch between two id's works perfectly fine.
    fn switch_normal() {
        let (state, settings, _tempdir) = get_test_state();

        let response = switch(&settings, &state, get_message(1, 2));

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
    /// Tasks cannot be switched with themselves.
    fn switch_task_with_itself() {
        let (state, settings, _tempdir) = get_test_state();

        let response = switch(&settings, &state, get_message(1, 1));

        // Response is correct
        assert!(matches!(response, Response::Failure(_)));
        if let Response::Failure(text) = response {
            assert_eq!(text, "You cannot switch a task with itself.");
        };
    }

    #[test]
    /// If any task that is specified as dependency get's switched,
    /// all dependants need to be updated.
    fn switch_task_with_dependant() {
        let (state, settings, _tempdir) = get_test_state();

        switch(&settings, &state, get_message(0, 3));

        let state = state.lock().unwrap();
        assert_eq!(state.tasks().get(&4).unwrap().dependencies, vec![0, 3]);
    }

    #[test]
    /// A task with two dependencies shouldn't experience any change, if those two dependencies
    /// switched places.
    fn switch_double_dependency() {
        let (state, settings, _tempdir) = get_test_state();

        switch(&settings, &state, get_message(1, 2));

        let state = state.lock().unwrap();
        assert_eq!(state.tasks().get(&5).unwrap().dependencies, vec![2]);
        assert_eq!(state.tasks().get(&6).unwrap().dependencies, vec![1, 3]);
    }

    #[test]
    /// You can only switch tasks that are either stashed or queued.
    /// Everything else should result in an error message.
    fn switch_invalid() {
        let (state, settings, _tempdir) = get_state();

        let combinations: Vec<(usize, usize)> = vec![
            (0, 1), // Queued + Done
            (0, 3), // Queued + Stashed
            (0, 4), // Queued + Running
            (0, 5), // Queued + Paused
            (2, 1), // Stashed + Done
            (2, 3), // Stashed + Stashed
            (2, 4), // Stashed + Running
            (2, 5), // Stashed + Paused
        ];

        for ids in combinations {
            let response = switch(&settings, &state, get_message(ids.0, ids.1));

            // Assert, that we get a failure with the correct text.
            assert!(matches!(response, Response::Failure(_)));
            if let Response::Failure(text) = response {
                assert_eq!(text, "Tasks have to be either queued or stashed.");
            };
        }
    }
}
