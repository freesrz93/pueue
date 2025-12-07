use pueue_lib::Client;

use super::{OutputStyle, get_state};
use crate::internal_prelude::*;

/// Print the full command of specified tasks in a single line.
/// This is useful for directly copying the command.
pub async fn print_cmd(
    client: &mut Client,
    _style: &OutputStyle,
    task_ids: Vec<usize>,
) -> Result<()> {
    let state = get_state(client).await?;

    for task_id in task_ids {
        let Some(task) = state.tasks.get(&task_id) else {
            eprintln!("Task {task_id} not found");
            continue;
        };

        // Print the command in a single line
        println!("{}", task.command);
    }

    Ok(())
}
