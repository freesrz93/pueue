use pueue_lib::{Client, message::*};

use super::handle_response;
use crate::{client::style::OutputStyle, internal_prelude::*};

/// Reverse the order of specified tasks in the queue.
///
/// This function takes a list of task IDs and reverses their order by generating
/// appropriate switch operations. For example, [1, 2, 3, 4] becomes [4, 3, 2, 1].
pub async fn reverse(client: &mut Client, style: &OutputStyle, task_ids: Vec<usize>) -> Result<()> {
    if task_ids.is_empty() {
        return Err(color_eyre::eyre::eyre!("Task ID list cannot be empty."));
    }

    if task_ids.len() == 1 {
        return Ok(());
    }

    // Create pairs to swap: first with last, second with second-to-last, etc.
    let mut task_ids_1 = Vec::new();
    let mut task_ids_2 = Vec::new();

    let mid = (task_ids.len() + 1) / 2;
    for i in 0..mid {
        let j = task_ids.len() - 1 - i;
        if i < j {
            task_ids_1.push(task_ids[i]);
            task_ids_2.push(task_ids[j]);
        }
    }

    if task_ids_1.is_empty() {
        return Ok(());
    }

    client
        .send_request(SwitchRequest {
            task_ids_1,
            task_ids_2,
        })
        .await?;

    let response = client.receive_response().await?;

    handle_response(style, response)
}
