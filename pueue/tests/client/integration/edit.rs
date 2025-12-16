use std::collections::HashMap;

use pueue_lib::{settings::EditMode, task::TaskStatus};

use crate::{client::helper::*, internal_prelude::*};

/// Test that editing a task without any flags only updates the command.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn edit_task_directory() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Create a stashed message which we'll edit later on.
    let mut message = create_add_message(shared, "before");
    message.stashed = true;
    send_request(shared, message)
        .await
        .context("Failed to to add stashed task.")?;

    // Update the task's command by piping a string to the temporary file.
    let mut envs = HashMap::new();
    envs.insert(
        "EDITOR",
        r#"echo "after" > "${PUEUE_EDIT_PATH}/0/command" ||"#,
    );
    run_client_command_with_env(shared, &["edit", "0"], envs)?.success()?;

    // Make sure that the command has been updated.
    let state = get_state(shared).await?;
    let task = state.tasks.get(&0).unwrap();
    assert_eq!(task.command, "after");

    // All other properties should be unchanged.
    assert_eq!(task.path, daemon.tempdir.path());
    assert_eq!(task.label, None);
    assert_eq!(task.priority, 0);

    Ok(())
}

/// Test that editing a multiple task properties works as expected.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn edit_all_task_properties() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Create a stashed message which we'll edit later on.
    let mut message = create_add_message(shared, "this is a test");
    message.stashed = true;
    send_request(shared, message)
        .await
        .context("Failed to to add stashed task.")?;

    // Update all task properties by piping a string to the respective temporary file.
    let mut envs = HashMap::new();
    envs.insert(
        "EDITOR",
        "echo 'command' > ${PUEUE_EDIT_PATH}/0/command && \
echo '/tmp' > ${PUEUE_EDIT_PATH}/0/path && \
echo 'label' > ${PUEUE_EDIT_PATH}/0/label && \
echo '5' > ${PUEUE_EDIT_PATH}/0/priority || ",
    );
    run_client_command_with_env(shared, &["edit", "0"], envs)?.success()?;

    // Make sure that all properties have been updated.
    let state = get_state(shared).await?;
    let task = state.tasks.get(&0).unwrap();
    assert_eq!(task.command, "command");
    assert_eq!(task.path.to_string_lossy(), "/tmp");
    assert_eq!(task.label, Some("label".to_string()));
    assert_eq!(task.priority, 5);

    Ok(())
}

/// Ensure that deleting the label in the editor result in the deletion of the task's label.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn edit_delete_label() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Create a stashed message which we'll edit later on.
    let mut message = create_add_message(shared, "this is a test");
    message.stashed = true;
    message.label = Some("Testlabel".to_owned());
    send_request(shared, message)
        .await
        .context("Failed to to add stashed task.")?;

    // Echo an empty string into the file.
    let mut envs = HashMap::new();
    envs.insert("EDITOR", "echo '' > ${PUEUE_EDIT_PATH}/0/label ||");
    run_client_command_with_env(shared, &["edit", "0"], envs)?.success()?;

    // Make sure that the label has indeed be deleted
    let state = get_state(shared).await?;
    let task = state.tasks.get(&0).unwrap();
    assert_eq!(task.label, None);

    Ok(())
}

/// Ensure that updating the priority in the editor results in the modification of the task's
/// priority.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn edit_change_priority() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Create a stashed message which we'll edit later on.
    let mut message = create_add_message(shared, "this is a test");
    message.stashed = true;
    message.priority = Some(0);
    send_request(shared, message)
        .await
        .context("Failed to to add stashed task.")?;

    // Echo a new priority into the file.
    let mut envs = HashMap::new();
    envs.insert("EDITOR", "echo '99' > ${PUEUE_EDIT_PATH}/0/priority ||");
    run_client_command_with_env(shared, &["edit", "0"], envs)?.success()?;

    // Make sure that the priority has indeed been updated.
    let state = get_state(shared).await?;
    let task = state.tasks.get(&0).unwrap();
    assert_eq!(task.priority, 99);

    Ok(())
}

/// Test that automatic restoration of a task's state works, if the edit command fails for some
/// reason.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fail_to_edit_task() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Create a stashed message which we'll edit later on.
    let mut message = create_add_message(shared, "this is a test");
    message.stashed = true;
    send_request(shared, message)
        .await
        .context("Failed to to add stashed task.")?;

    // Run a editor command that crashes.
    let mut envs = HashMap::new();
    envs.insert("EDITOR", "non_existing_test_binary");
    let output = run_client_command_with_env(shared, &["edit", "0"], envs)?.failure()?;
    assert!(
        !output.status.success(),
        "The command should fail, as the command isn't valid"
    );

    // Make sure that nothing has changed and the task is `Stashed` again.
    let state = get_state(shared).await?;
    let task = state.tasks.get(&0).unwrap();
    assert_eq!(task.command, "this is a test");
    assert_eq!(task.status, TaskStatus::Stashed { enqueue_at: None });

    Ok(())
}

/// Test that editing a task without any flags only updates the command.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn edit_task_toml() -> Result<()> {
    // Overwrite the edit mode to toml.
    let (mut settings, tempdir) = daemon_base_setup()?;
    settings.client.edit_mode = EditMode::Toml;
    settings.save(&Some(tempdir.path().join("pueue.yml")))?;
    let daemon = daemon_with_settings(settings, tempdir).await?;
    let shared = &daemon.settings.shared;

    // Create a stashed message which we'll edit later on.
    let mut message = create_add_message(shared, "this is a test");
    message.stashed = true;
    send_request(shared, message)
        .await
        .context("Failed to to add stashed task.")?;

    // Update the task's command by piping a string to the temporary file.
    let mut envs = HashMap::new();
    envs.insert(
        "EDITOR",
        "echo '[0]\nid = 0\ncommand = \"expected command string\"\npath = \"/tmp\"\npriority = 0\ndependencies = []' > ${PUEUE_EDIT_PATH} ||",
    );
    run_client_command_with_env(shared, &["edit", "0"], envs)?.success()?;

    // Make sure that both the command and the path has been updated.
    let state = get_state(shared).await?;
    let task = state.tasks.get(&0).unwrap();
    assert_eq!(task.command, "expected command string");
    assert_eq!(task.path.to_string_lossy(), "/tmp");

    // All other properties should be unchanged.
    assert_eq!(task.label, None);
    assert_eq!(task.priority, 0);

    Ok(())
}

/// While editing, the original commands should be used instead of the substituted aliased command
/// strings.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn edit_with_alias() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Create the alias file.
    let mut aliases = HashMap::new();
    aliases.insert("before".into(), "before aliased".into());
    aliases.insert("after".into(), "after aliased".into());
    create_test_alias_file(daemon.tempdir.path(), aliases)?;

    // Create a stashed message which we'll edit later on.
    let mut message = create_add_message(shared, "before");
    message.stashed = true;
    send_request(shared, message)
        .await
        .context("Failed to to add stashed task.")?;

    // Update the task's command by piping a string to the temporary file.
    // However, make sure that the old command is `before` and not the aliased command!
    let mut envs = HashMap::new();
    envs.insert(
        "EDITOR",
        r#"[[ "$(cat ${PUEUE_EDIT_PATH}/0/command)" == "before" ]] \
&& echo "after" > "${PUEUE_EDIT_PATH}/0/command" ||"#,
    );
    run_client_command_with_env(shared, &["edit", "0"], envs)?.success()?;

    // Make sure that the command has been updated and the aliase worked.
    let state = get_state(shared).await?;
    let task = state.tasks.get(&0).unwrap();
    assert_eq!(task.original_command, "after");
    assert_eq!(task.command, "after aliased");

    // All other properties should be unchanged.
    assert_eq!(task.path, daemon.tempdir.path());
    assert_eq!(task.label, None);
    assert_eq!(task.priority, 0);

    Ok(())
}

/// Test that editing task dependencies works as expected.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn edit_add_dependencies() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Create three stashed tasks
    for i in 0..3 {
        let mut message = create_add_message(shared, &format!("task {}", i));
        message.stashed = true;
        send_request(shared, message)
            .await
            .context("Failed to add stashed task.")?;
    }

    // Edit task 2 to depend on tasks 0 and 1
    let mut envs = HashMap::new();
    envs.insert(
        "EDITOR",
        "echo '0 1' > ${PUEUE_EDIT_PATH}/2/dependencies ||",
    );
    run_client_command_with_env(shared, &["edit", "2"], envs)?.success()?;

    // Verify the dependencies were set correctly
    let state = get_state(shared).await?;
    let task = state.tasks.get(&2).unwrap();
    assert_eq!(task.dependencies, vec![0, 1]);

    Ok(())
}

/// Test that editing dependencies with comma-separated values works.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn edit_dependencies_comma_separated() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Create three stashed tasks
    for i in 0..3 {
        let mut message = create_add_message(shared, &format!("task {}", i));
        message.stashed = true;
        send_request(shared, message)
            .await
            .context("Failed to add stashed task.")?;
    }

    // Edit task 2 to depend on tasks 0 and 1 using comma-separated format
    let mut envs = HashMap::new();
    envs.insert(
        "EDITOR",
        "echo '0, 1' > ${PUEUE_EDIT_PATH}/2/dependencies ||",
    );
    run_client_command_with_env(shared, &["edit", "2"], envs)?.success()?;

    // Verify the dependencies were set correctly
    let state = get_state(shared).await?;
    let task = state.tasks.get(&2).unwrap();
    assert_eq!(task.dependencies, vec![0, 1]);

    Ok(())
}

/// Test that clearing dependencies by leaving the file empty works.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn edit_clear_dependencies() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Create two tasks, with task 1 depending on task 0
    let mut message = create_add_message(shared, "task 0");
    message.stashed = true;
    send_request(shared, message)
        .await
        .context("Failed to add stashed task.")?;

    let mut message = create_add_message(shared, "task 1");
    message.stashed = true;
    message.dependencies = vec![0];
    send_request(shared, message)
        .await
        .context("Failed to add stashed task.")?;

    // Verify initial dependency
    let state = get_state(shared).await?;
    let task = state.tasks.get(&1).unwrap();
    assert_eq!(task.dependencies, vec![0]);

    // Clear the dependencies by writing empty string
    let mut envs = HashMap::new();
    envs.insert("EDITOR", "echo '' > ${PUEUE_EDIT_PATH}/1/dependencies ||");
    run_client_command_with_env(shared, &["edit", "1"], envs)?.success()?;

    // Verify dependencies were cleared
    let state = get_state(shared).await?;
    let task = state.tasks.get(&1).unwrap();
    assert_eq!(task.dependencies, Vec::<usize>::new());

    Ok(())
}

/// Test that self-dependency is rejected.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn edit_reject_self_dependency() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Create a stashed task
    let mut message = create_add_message(shared, "task 0");
    message.stashed = true;
    send_request(shared, message)
        .await
        .context("Failed to add stashed task.")?;

    // Try to make task depend on itself
    let mut envs = HashMap::new();
    envs.insert("EDITOR", "echo '0' > ${PUEUE_EDIT_PATH}/0/dependencies ||");
    let output = run_client_command_with_env(shared, &["edit", "0"], envs)?.failure()?;

    // Verify it failed
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("cannot depend on itself"),
        "Expected self-dependency error, got: {}",
        stderr
    );

    Ok(())
}

/// Test that non-existent dependency is rejected.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn edit_reject_nonexistent_dependency() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Create a stashed task
    let mut message = create_add_message(shared, "task 0");
    message.stashed = true;
    send_request(shared, message)
        .await
        .context("Failed to add stashed task.")?;

    // Try to make task depend on non-existent task 999
    let mut envs = HashMap::new();
    envs.insert(
        "EDITOR",
        "echo '999' > ${PUEUE_EDIT_PATH}/0/dependencies ||",
    );
    let output = run_client_command_with_env(shared, &["edit", "0"], envs)?.failure()?;

    // Verify it failed
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not found") || stderr.contains("999"),
        "Expected non-existent dependency error, got: {}",
        stderr
    );

    Ok(())
}

/// Test that circular dependencies are detected and rejected.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn edit_reject_circular_dependency() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Create two tasks where task 1 depends on task 0
    let mut message = create_add_message(shared, "task 0");
    message.stashed = true;
    send_request(shared, message)
        .await
        .context("Failed to add stashed task.")?;

    let mut message = create_add_message(shared, "task 1");
    message.stashed = true;
    message.dependencies = vec![0];
    send_request(shared, message)
        .await
        .context("Failed to add stashed task.")?;

    // Try to make task 0 depend on task 1 (creating a cycle)
    let mut envs = HashMap::new();
    envs.insert("EDITOR", "echo '1' > ${PUEUE_EDIT_PATH}/0/dependencies ||");
    let output = run_client_command_with_env(shared, &["edit", "0"], envs)?.failure()?;

    // Verify it failed with circular dependency error
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Circular dependency") || stderr.contains("circular"),
        "Expected circular dependency error, got: {}",
        stderr
    );

    Ok(())
}

/// Test that dependencies are automatically sorted and deduplicated.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn edit_dependencies_sort_and_dedup() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Create four stashed tasks
    for i in 0..4 {
        let mut message = create_add_message(shared, &format!("task {}", i));
        message.stashed = true;
        send_request(shared, message)
            .await
            .context("Failed to add stashed task.")?;
    }

    // Edit task 3 with unsorted and duplicate dependencies
    let mut envs = HashMap::new();
    envs.insert(
        "EDITOR",
        "echo '2 0 1 2 0' > ${PUEUE_EDIT_PATH}/3/dependencies ||",
    );
    run_client_command_with_env(shared, &["edit", "3"], envs)?.success()?;

    // Verify dependencies were sorted and deduplicated
    let state = get_state(shared).await?;
    let task = state.tasks.get(&3).unwrap();
    assert_eq!(task.dependencies, vec![0, 1, 2]);

    Ok(())
}

/// Test editing dependencies in TOML mode.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn edit_dependencies_toml() -> Result<()> {
    // Overwrite the edit mode to toml.
    let (mut settings, tempdir) = daemon_base_setup()?;
    settings.client.edit_mode = EditMode::Toml;
    settings.save(&Some(tempdir.path().join("pueue.yml")))?;
    let daemon = daemon_with_settings(settings, tempdir).await?;
    let shared = &daemon.settings.shared;

    // Create three stashed tasks
    for i in 0..3 {
        let mut message = create_add_message(shared, &format!("task {}", i));
        message.stashed = true;
        send_request(shared, message)
            .await
            .context("Failed to add stashed task.")?;
    }

    // Edit task 2 to depend on tasks 0 and 1 using TOML format
    let mut envs = HashMap::new();
    envs.insert(
        "EDITOR",
        "echo '[2]\nid = 2\ncommand = \"task 2\"\npath = \"/tmp\"\npriority = 0\ndependencies = [0, 1]' > ${PUEUE_EDIT_PATH} ||",
    );
    run_client_command_with_env(shared, &["edit", "2"], envs)?.success()?;

    // Verify the dependencies were set correctly
    let state = get_state(shared).await?;
    let task = state.tasks.get(&2).unwrap();
    assert_eq!(task.dependencies, vec![0, 1]);

    Ok(())
}
