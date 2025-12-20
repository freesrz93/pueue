use std::{
    collections::{BTreeMap, HashMap},
    env::vars,
    fs::read_to_string,
    path::PathBuf,
};

use pueue_lib::{Client, Settings, error::Error, message::*, state::PUEUE_DEFAULT_GROUP};
use serde::Deserialize;
use tempfile::tempdir;

use crate::{
    client::{commands::edit::run_editor, style::OutputStyle},
    internal_prelude::*,
};

/// A simplified version of EditableTask for importing.
///
/// The `id` field is not required and will be ignored if present.
/// New task IDs will be automatically assigned by the daemon.
#[derive(Debug, Deserialize)]
struct ImportableTask {
    #[serde(rename = "command")]
    original_command: String,
    path: PathBuf,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    priority: i32,
    #[serde(default)]
    dependencies: Vec<usize>,
}

/// Import tasks from TOML format.
///
/// If no file is specified, opens an empty TOML file in the user's editor.
/// After closing the editor, the tasks will be imported.
pub async fn import(
    client: &mut Client,
    settings: Settings,
    _style: &OutputStyle,
    file: Option<PathBuf>,
    group: Option<String>,
) -> Result<()> {
    let target_group = group.unwrap_or_else(|| PUEUE_DEFAULT_GROUP.to_string());

    // Read the TOML content either from file or from editor
    let content = if let Some(file_path) = file {
        // Read from file
        read_to_string(&file_path)
            .map_err(|err| Error::IoPathError(file_path.clone(), "reading import file", err))?
    } else {
        // Open editor with empty TOML template
        let temp_dir = tempdir().context("Failed to create temporary directory for import.")?;
        let temp_file_path = temp_dir.path().join("import.toml");

        // Create an empty template TOML file with an example
        let template = r#"# Import tasks by defining them below in TOML format.
# Each task should have a unique key (can be any string, will be ignored).
# 
# Note: The 'id' field is optional and will be ignored if present.
#       New task IDs will be automatically assigned by the daemon.
# 
# Example:
# [task1]
# command = "echo 'Hello, World!'"
# path = "/home/user"
# label = "My Task"
# priority = 0
# dependencies = []

"#;
        std::fs::write(&temp_file_path, template).map_err(|err| {
            Error::IoPathError(temp_file_path.clone(), "creating temporary file", err)
        })?;

        // Open the editor
        run_editor(&settings, &temp_file_path)?;

        // Read the content back
        read_to_string(&temp_file_path).map_err(|err| {
            Error::IoPathError(temp_file_path.clone(), "reading temporary file", err)
        })?
    };

    // Parse the TOML content
    let map: BTreeMap<String, ImportableTask> =
        toml::from_str(&content).context("Failed to deserialize TOML. Please check the format.")?;

    if map.is_empty() {
        println!("No tasks to import.");
        return Ok(());
    }

    // Convert ImportableTask to AddRequest for each task
    let mut imported_count = 0;
    for (_, importable_task) in map {
        // Create the add message with stashed status
        let message = Request::Add(AddRequest {
            command: importable_task.original_command.clone(),
            path: importable_task.path.clone(),
            // Catch the current environment for later injection into the task's process.
            envs: HashMap::from_iter(vars()),
            start_immediately: false,
            stashed: true,
            group: target_group.clone(),
            enqueue_at: None,
            dependencies: importable_task.dependencies.clone(),
            priority: Some(importable_task.priority),
            label: importable_task.label.clone(),
        });

        // Send the request to add the task
        client.send_request(message).await?;
        let response = client.receive_response().await?;

        // Check if the task was added successfully
        match response {
            Response::AddedTask(..) => {
                imported_count += 1;
            }
            Response::Failure(text) => {
                eprintln!("Failed to import task: {}", text);
            }
            _ => {
                eprintln!("Unexpected response while importing task");
            }
        }
    }

    println!(
        "Successfully imported {} task(s) to group '{}'",
        imported_count, target_group
    );

    Ok(())
}
