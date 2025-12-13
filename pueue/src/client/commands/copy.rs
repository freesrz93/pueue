use std::collections::BTreeMap;

use chrono::Local;
use color_eyre::eyre::ContextCompat;
use pueue_lib::{
    Client, Settings,
    message::*,
    task::{Task, TaskStatus},
};

use crate::{
    client::commands::{edit::edit_tasks, get_state},
    internal_prelude::*,
};

/// Copy existing tasks, creating new tasks with the same command and configuration.
/// Unlike restart, this works on tasks in any state.
/// The copied tasks will have " (copy from #N)" appended to their label.
pub async fn copy(
    client: &mut Client,
    settings: Settings,
    task_ids: Vec<usize>,
    start_immediately: bool,
    enqueue: bool,
    edit: bool,
) -> Result<()> {
    if task_ids.is_empty() {
        bail!("Please provide the ids of the tasks you want to copy.");
    }

    // By default, copied tasks are stashed unless --enqueue is specified
    let new_status = if enqueue {
        TaskStatus::Queued {
            enqueued_at: Local::now(),
        }
    } else {
        TaskStatus::Stashed { enqueue_at: None }
    };

    let state = get_state(client).await?;

    // Filter to get the specified tasks - copy works on all tasks regardless of state
    let all_filter = |_task: &Task| true;
    let filtered_tasks = state.filter_tasks(all_filter, Some(task_ids));

    // Get all tasks that should be copied.
    let mut tasks: BTreeMap<usize, Task> = filtered_tasks
        .matching_ids
        .iter()
        .map(|task_id| (*task_id, state.tasks.get(task_id).unwrap().clone()))
        .collect();

    // If the tasks should be edited, edit them in one go.
    if edit {
        let mut editable_tasks: Vec<EditableTask> =
            tasks.values().map(EditableTask::from).collect();
        editable_tasks = edit_tasks(&settings, editable_tasks)?;

        // Now merge the edited properties back into the tasks.
        for edited in editable_tasks {
            let task = tasks.get_mut(&edited.id).context(format!(
                "Found unexpected task id during editing: {}",
                edited.id
            ))?;
            edited.into_task(task);
        }
    }

    // Go through all tasks we found and create new tasks from them.
    for (_, mut task) in tasks {
        task.status = new_status.clone();

        // Append " (copy from #N)" to the label to indicate the source
        let copy_label = if let Some(label) = &task.label {
            format!("{} (copy from #{})", label, task.id)
        } else {
            format!("(copy from #{})", task.id)
        };

        // Create a request to send the new task to the daemon.
        let add_task_message = AddRequest {
            command: task.original_command,
            path: task.path,
            envs: task.envs.clone(),
            start_immediately,
            stashed: !enqueue,
            group: task.group.clone(),
            enqueue_at: None,
            dependencies: Vec::new(),
            priority: Some(task.priority),
            label: Some(copy_label),
        };

        // Send the copied task to the daemon and abort on any failure messages.
        client.send_request(add_task_message).await?;
        if let Response::Failure(message) = client.receive_response().await? {
            bail!(message);
        };
    }

    if !filtered_tasks.matching_ids.is_empty() {
        println!("Copied tasks: {:?}", filtered_tasks.matching_ids);
    }
    if !filtered_tasks.non_matching_ids.is_empty() {
        eprintln!("Couldn't find tasks: {:?}", filtered_tasks.non_matching_ids);
    }

    Ok(())
}
