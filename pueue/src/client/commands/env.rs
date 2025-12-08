use pueue_lib::{
    Client,
    message::{EnvRequest, Response},
};

use super::handle_response;
use crate::{
    client::{cli::EnvCommand, style::OutputStyle},
    internal_prelude::*,
};

/// Set or unset an environment variable on a task.
pub async fn env(client: &mut Client, style: &OutputStyle, cmd: EnvCommand) -> Result<()> {
    let request = match cmd {
        EnvCommand::Set {
            task_id,
            key,
            value,
        } => EnvRequest::Set {
            task_id,
            key,
            value,
        },
        EnvCommand::Unset { task_id, key } => EnvRequest::Unset { task_id, key },
        EnvCommand::List { task_id } => EnvRequest::List { task_id },
    };

    client.send_request(request).await?;

    let response = client.receive_response().await?;

    if let Response::EnvVars(env_response) = response {
        for (key, value) in env_response.envs {
            println!("{key}={value}");
        }

        return Ok(());
    }

    handle_response(style, response)
}
