use pueue_lib::{Client, message::*};

use super::handle_response;
use crate::{client::style::OutputStyle, internal_prelude::*};

/// Switch queued or stashed tasks by swapping two lists pairwise.
pub async fn switch(
    client: &mut Client,
    style: &OutputStyle,
    task_ids_1: Vec<usize>,
    task_ids_2: Vec<usize>,
) -> Result<()> {
    // Validate that both lists have the same length
    if task_ids_1.len() != task_ids_2.len() {
        return Err(color_eyre::eyre::eyre!(
            "Both task ID lists must have the same length. Got {} and {} tasks.",
            task_ids_1.len(),
            task_ids_2.len()
        ));
    }

    if task_ids_1.is_empty() {
        return Err(color_eyre::eyre::eyre!("Task ID lists cannot be empty."));
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
