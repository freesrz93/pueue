use pueue_lib::{Settings, Task, TaskStatus, log::clean_log_handles, message::*};

use super::ok_or_failure_message;
use crate::{
    daemon::{internal_state::SharedState, network::response_helper::*},
    ok_or_save_state_failure,
};

/// Invoked when calling `pueue remove`.
/// Remove tasks from the queue.
/// We have to ensure that those tasks aren't running!
pub fn remove(settings: &Settings, state: &SharedState, task_ids: Vec<usize>) -> Response {
    let mut state = state.lock().unwrap();

    // Filter all running tasks, since we cannot remove them.
    let filter = |task: &Task| {
        matches!(
            task.status,
            TaskStatus::Queued { .. }
                | TaskStatus::Stashed { .. }
                | TaskStatus::Done { .. }
                | TaskStatus::Locked { .. }
        )
    };
    let mut filtered_tasks = state.filter_tasks(filter, Some(task_ids));

    // Don't delete tasks, if there are other tasks that depend on this one.
    // However, we allow to delete those tasks, if they're supposed to be deleted as well.
    for task_id in filtered_tasks.matching_ids.clone() {
        if !state.is_task_removable(&task_id, &filtered_tasks.matching_ids) {
            filtered_tasks.non_matching_ids.push(task_id);
            filtered_tasks.matching_ids.retain(|id| id != &task_id);
        };
    }

    for task_id in &filtered_tasks.matching_ids {
        // Before removing the task, clean up reverse dependencies
        if let Some(task) = state.tasks().get(task_id) {
            // Remove this task from the dependents lists of its dependencies
            let dependencies = task.dependencies.clone();
            for &dep_id in &dependencies {
                if let Some(dep_task) = state.tasks_mut().get_mut(&dep_id) {
                    dep_task.dependents.retain(|&id| id != *task_id);
                }
            }
        }

        state.tasks_mut().remove(task_id);

        clean_log_handles(*task_id, &settings.shared.pueue_directory());
    }

    ok_or_save_state_failure!(state.save(settings));

    compile_task_response("Tasks removed from list", filtered_tasks)
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::{super::fixtures::*, *};

    #[test]
    fn normal_remove() {
        let (state, settings, _tempdir) = get_stub_state();

        // 3 and 4 aren't allowed to be removed, since they're running.
        // The rest will succeed.
        let response = remove(&settings, &state, vec![0, 1, 2, 3, 4]);

        // Response is correct
        assert!(matches!(response, Response::Success(_)));
        if let Response::Success(text) = response {
            assert_eq!(
                text,
                "Tasks removed from list: 0, 1, 2\nThe command failed for tasks: 3, 4"
            );
        };

        let state = state.lock().unwrap();
        assert_eq!(state.tasks().len(), 2);
    }

    #[test]
    fn removal_of_dependencies() {
        let (state, settings, _tempdir) = get_stub_state();

        {
            let mut state = state.lock().unwrap();
            // Add a task with a dependency to a finished task
            let mut task = get_stub_task("5", StubStatus::Queued);
            task.dependencies = vec![1];
            let task_id = state.add_task(task);

            // Update reverse dependency index
            if let Some(dep_task) = state.tasks_mut().get_mut(&1) {
                dep_task.dependents.push(task_id);
            }

            // Add a task depending on the previous task -> Linked dependencies
            let mut task = get_stub_task("6", StubStatus::Queued);
            task.dependencies = vec![5];
            let task_id = state.add_task(task);

            // Update reverse dependency index
            if let Some(dep_task) = state.tasks_mut().get_mut(&5) {
                dep_task.dependents.push(task_id);
            }
        }

        // Make sure we cannot remove a task with dependencies.
        let response = remove(&settings, &state, vec![1]);

        // Response is correct
        assert!(matches!(response, Response::Failure(_)));
        if let Response::Failure(text) = response {
            assert_eq!(text, "The command failed for tasks: 1");
        };

        {
            let state = state.lock().unwrap();
            assert_eq!(state.tasks().len(), 7);
        }

        // Make sure we cannot remove a task with recursive dependencies.
        let response = remove(&settings, &state, vec![1, 5]);

        // Response is correct
        assert!(matches!(response, Response::Failure(_)));
        if let Response::Failure(text) = response {
            assert_eq!(text, "The command failed for tasks: 1, 5");
        };

        {
            let state = state.lock().unwrap();
            assert_eq!(state.tasks().len(), 7);
        }

        // Make sure we can remove tasks with dependencies if all dependencies are specified.
        let response = remove(&settings, &state, vec![1, 5, 6]);

        // Response is correct
        assert!(matches!(response, Response::Success(_)));
        if let Response::Success(text) = response {
            assert_eq!(text, "Tasks removed from list: 1, 5, 6");
        };

        {
            let state = state.lock().unwrap();
            assert_eq!(state.tasks().len(), 4);
        }
    }
}
