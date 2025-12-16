use std::collections::BTreeMap;

use pueue_lib::{
    Client, Settings, State,
    message::{AddRequest, GroupRequest, Response},
    state::PUEUE_ARCHIVE_GROUP,
    task::{Task, TaskStatus},
};

use super::{get_state, handle_user_confirmation, remove::remove, state::state};
use crate::{client::style::OutputStyle, internal_prelude::*};

/// Archive tasks by copying them into the hidden `archive` group and removing the originals.
/// Calling the command without any task ids shows the archive group.
pub async fn archive(
    client: &mut Client,
    settings: Settings,
    style: &OutputStyle,
    task_ids: Vec<usize>,
    json: bool,
) -> Result<()> {
    // Without ids we simply display the archive group.
    if task_ids.is_empty() {
        return state(
            client,
            settings,
            style,
            Vec::new(),
            json,
            Some(PUEUE_ARCHIVE_GROUP.to_string()),
        )
        .await;
    }

    // Fetch current state to validate and prepare tasks.
    let state = get_state(client).await?;

    // Figure out which tasks actually exist.
    let all_filter = |_task: &Task| true;
    let filtered_tasks = state.filter_tasks(all_filter, Some(task_ids.clone()));

    // Collect the concrete task structs for ids that exist.
    let candidate_tasks: BTreeMap<usize, Task> = filtered_tasks
        .matching_ids
        .iter()
        .filter_map(|task_id| {
            state
                .tasks
                .get(task_id)
                .cloned()
                .map(|task| (*task_id, task))
        })
        .collect();

    // Only allow tasks that can be removed (mirrors daemon side logic) and have no blocking dependants.
    let mut blocked_ids = Vec::new();
    let mut status_ok: BTreeMap<usize, Task> = BTreeMap::new();
    for (task_id, task) in candidate_tasks {
        if is_removable_status(&task) {
            status_ok.insert(task_id, task);
        } else {
            blocked_ids.push(task_id);
        }
    }

    let mut archivable = status_ok;
    loop {
        let current_ids: Vec<usize> = archivable.keys().copied().collect();
        let to_remove: Vec<usize> = current_ids
            .iter()
            .copied()
            .filter(|task_id| has_blocking_dependant(*task_id, &state, &current_ids))
            .collect();

        if to_remove.is_empty() {
            break;
        }

        for task_id in to_remove {
            if archivable.remove(&task_id).is_some() {
                blocked_ids.push(task_id);
            }
        }
    }

    if archivable.is_empty() {
        if !filtered_tasks.non_matching_ids.is_empty() {
            bail!("Couldn't find tasks: {:?}", filtered_tasks.non_matching_ids);
        }
        bail!(
            "No tasks can be archived. Skipped tasks: {:?}. Make sure they are finished, queued, stashed or unlocked and not depended on by running tasks.",
            blocked_ids
        );
    }

    if settings.client.show_confirmation_questions {
        handle_user_confirmation("archive", &archivable.keys().copied().collect::<Vec<_>>())?;
    }

    // Ensure the archive group exists before enqueuing new tasks.
    if !state.groups.contains_key(PUEUE_ARCHIVE_GROUP) {
        client
            .send_request(GroupRequest::Add {
                name: PUEUE_ARCHIVE_GROUP.to_string(),
                parallel_tasks: None,
            })
            .await?;

        match client.receive_response().await? {
            Response::Success(_) | Response::Group(_) => {}
            Response::Failure(message) => bail!(message),
            other => bail!("Unexpected response while creating archive group: {other:?}"),
        }
    }

    let mut archived_sources = Vec::new();
    let mut new_task_ids = Vec::new();

    for (_, task) in archivable.iter() {
        let label = task
            .label
            .clone()
            .map(|label| format!("{label} (archived from #{})", task.id))
            .or_else(|| Some(format!("(archived from #{})", task.id)));

        let add_task_message = AddRequest {
            command: task.original_command.clone(),
            path: task.path.clone(),
            envs: task.envs.clone(),
            start_immediately: false,
            stashed: true,
            group: PUEUE_ARCHIVE_GROUP.to_string(),
            enqueue_at: None,
            dependencies: Vec::new(),
            priority: Some(task.priority),
            label,
        };

        client.send_request(add_task_message).await?;
        match client.receive_response().await? {
            Response::AddedTask(response) => {
                archived_sources.push(task.id);
                new_task_ids.push(response.task_id);
            }
            Response::Failure(message) => bail!(message),
            other => bail!("Unexpected response while archiving: {other:?}"),
        }
    }

    // Remove the original tasks now that copies are in the archive.
    remove(client, settings, style, archived_sources.clone()).await?;

    if !new_task_ids.is_empty() {
        println!(
            "Archived tasks: {:?} (archived copies are stashed in group \"{}\", ids: {:?})",
            archived_sources, PUEUE_ARCHIVE_GROUP, new_task_ids
        );
    }

    if !filtered_tasks.non_matching_ids.is_empty() {
        eprintln!("Couldn't find tasks: {:?}", filtered_tasks.non_matching_ids);
    }

    if !blocked_ids.is_empty() {
        eprintln!(
            "Skipped tasks that cannot be archived (running, paused, or required by active dependants): {:?}",
            blocked_ids
        );
    }

    Ok(())
}

fn is_removable_status(task: &Task) -> bool {
    matches!(
        task.status,
        TaskStatus::Queued { .. }
            | TaskStatus::Stashed { .. }
            | TaskStatus::Done { .. }
            | TaskStatus::Locked { .. }
    )
}

fn has_blocking_dependant(task_id: usize, state: &State, targets: &[usize]) -> bool {
    state.tasks.values().any(|task| {
        task.dependencies.contains(&task_id)
            && !matches!(task.status, TaskStatus::Done { .. })
            && !targets.contains(&task.id)
    })
}
