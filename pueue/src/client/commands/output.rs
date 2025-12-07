use pueue_lib::{Client, log::get_log_path};

use super::{OutputStyle, get_state};
use crate::internal_prelude::*;

/// Print the full path to the output file of specified tasks.
/// This is useful for checking the log file directly.
pub async fn print_output(
    client: &mut Client,
    settings: pueue_lib::Settings,
    _style: &OutputStyle,
    task_ids: Vec<usize>,
) -> Result<()> {
    let state = get_state(client).await?;

    for task_id in task_ids {
        let Some(_task) = state.tasks.get(&task_id) else {
            eprintln!("Task {task_id} not found");
            continue;
        };

        // Get the log file path for this task
        let log_path = get_log_path(task_id, &settings.shared.pueue_directory());

        // Print the full path
        println!("{}", log_path.display());
    }

    Ok(())
}
