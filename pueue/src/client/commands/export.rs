use pueue_lib::{Client, message::*, state::PUEUE_DEFAULT_GROUP, task::Task};
use toml::map::Map;

use crate::{client::commands::get_state, internal_prelude::*};

/// Export tasks to TOML format.
///
/// By default, exports all tasks from the default group.
/// The output is printed to stdout and can be redirected to a file.
pub async fn export(
    client: &mut Client,
    task_ids: Vec<usize>,
    group: Option<String>,
) -> Result<()> {
    let state = get_state(client).await?;

    // Determine which tasks to export
    let tasks_to_export: Vec<&Task> = if !task_ids.is_empty() {
        // Export specific task IDs
        task_ids
            .iter()
            .filter_map(|id| state.tasks.get(id))
            .collect()
    } else {
        // Export all tasks from the specified group (or default group)
        let target_group = group.unwrap_or_else(|| PUEUE_DEFAULT_GROUP.to_string());
        state
            .tasks
            .values()
            .filter(|task| task.group == target_group)
            .collect()
    };

    if tasks_to_export.is_empty() {
        bail!("No tasks found to export.");
    }

    // Convert tasks to EditableTask and sort by natural order
    let mut editable_tasks: Vec<EditableTask> = tasks_to_export
        .iter()
        .map(|task| EditableTask::from(*task))
        .collect();

    // Sort by task ID using natural order
    editable_tasks.sort_by_key(|task| task.id);

    // Convert to map with string keys for TOML serialization
    // Using toml::map::Map with preserve_order feature to maintain insertion order
    let mut map = Map::new();
    for task in editable_tasks {
        let id_str = task.id.to_string();
        let value = toml::Value::try_from(&task).context("Failed to convert task to TOML value")?;
        map.insert(id_str, value);
    }

    // Serialize to TOML and print to stdout
    let toml = toml::to_string(&map).context("Failed to serialize tasks to TOML")?;

    println!("{}", toml);

    Ok(())
}
