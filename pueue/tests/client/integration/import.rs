use std::collections::HashMap;
use std::fs;

use pueue_lib::task::TaskStatus;

use crate::{client::helper::*, internal_prelude::*};

/// Test that importing tasks from a file creates the tasks correctly.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn import_from_file() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Create a TOML file with tasks
    let toml_content = r#"
[task1]
command = "echo 'imported task 1'"
path = "/tmp"
priority = 0
dependencies = []

[task2]
command = "echo 'imported task 2'"
path = "/tmp"
label = "test-label"
priority = 5
dependencies = []
"#;

    let temp_file = daemon.tempdir.path().join("import_test.toml");
    fs::write(&temp_file, toml_content)?;

    // Import the tasks
    run_client_command(shared, &["import", "-f", temp_file.to_str().unwrap()])?.success()?;

    // Verify the tasks were created
    let state = get_state(shared).await?;
    assert_eq!(state.tasks.len(), 2, "Should have imported 2 tasks");

    // Check first task
    let task0 = state.tasks.get(&0).unwrap();
    assert_eq!(task0.original_command, "echo 'imported task 1'");
    assert!(
        matches!(task0.status, TaskStatus::Stashed { .. }),
        "Imported tasks should be stashed"
    );

    // Check second task with label and priority
    let task1 = state.tasks.get(&1).unwrap();
    assert_eq!(task1.original_command, "echo 'imported task 2'");
    assert_eq!(task1.label, Some("test-label".to_string()));
    assert_eq!(task1.priority, 5);

    Ok(())
}

/// Test importing tasks to a specific group.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn import_to_specific_group() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Create a new group
    run_client_command(shared, &["group", "add", "importgroup"])?.success()?;

    // Create a TOML file with a task
    let toml_content = r#"
[task1]
command = "echo 'test'"
path = "/tmp"
priority = 0
dependencies = []
"#;

    let temp_file = daemon.tempdir.path().join("import_group.toml");
    fs::write(&temp_file, toml_content)?;

    // Import to the specific group
    run_client_command(
        shared,
        &[
            "import",
            "-g",
            "importgroup",
            "-f",
            temp_file.to_str().unwrap(),
        ],
    )?
    .success()?;

    // Verify the task was added to the correct group
    let state = get_state(shared).await?;
    let task = state.tasks.get(&0).unwrap();
    assert_eq!(task.group, "importgroup");

    Ok(())
}

/// Test importing tasks with dependencies.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn import_with_dependencies() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // First create some existing tasks to use as dependencies
    assert_success(add_task(shared, "echo 'base task 1'").await?);
    assert_success(add_task(shared, "echo 'base task 2'").await?);

    // Create a TOML file with a task that depends on existing tasks
    let toml_content = r#"
[task1]
command = "echo 'dependent task'"
path = "/tmp"
priority = 0
dependencies = [0, 1]
"#;

    let temp_file = daemon.tempdir.path().join("import_deps.toml");
    fs::write(&temp_file, toml_content)?;

    // Import the task
    run_client_command(shared, &["import", "-f", temp_file.to_str().unwrap()])?.success()?;

    // Verify the dependencies were preserved
    let state = get_state(shared).await?;
    let task = state.tasks.get(&2).unwrap();
    assert_eq!(task.dependencies, vec![0, 1]);

    Ok(())
}

/// Test importing an empty file or file with no tasks.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn import_empty_file() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Create an empty TOML file
    let toml_content = "# Empty file\n";
    let temp_file = daemon.tempdir.path().join("empty.toml");
    fs::write(&temp_file, toml_content)?;

    // Import should succeed but not create any tasks
    let output =
        run_client_command(shared, &["import", "-f", temp_file.to_str().unwrap()])?.success()?;

    let stdout = std::str::from_utf8(&output.stdout)?;
    assert!(
        stdout.contains("No tasks to import"),
        "Should show 'No tasks to import' message"
    );

    // Verify no tasks were created
    let state = get_state(shared).await?;
    assert_eq!(state.tasks.len(), 0, "Should have no tasks");

    Ok(())
}

/// Test importing tasks with special characters.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn import_with_special_characters() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Create a TOML file with special characters in command
    let toml_content = r#"
[task1]
command = "echo 'test \"quoted\" && echo $VAR'"
path = "/tmp"
priority = 0
dependencies = []
"#;

    let temp_file = daemon.tempdir.path().join("special_chars.toml");
    fs::write(&temp_file, toml_content)?;

    // Import the task
    run_client_command(shared, &["import", "-f", temp_file.to_str().unwrap()])?.success()?;

    // Verify the command was imported correctly
    let state = get_state(shared).await?;
    let task = state.tasks.get(&0).unwrap();
    assert_eq!(task.original_command, "echo 'test \"quoted\" && echo $VAR'");

    Ok(())
}

/// Test importing via editor (without -f flag).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn import_via_editor() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Create a mock editor that writes a task to the temporary file
    let toml_content = r#"[task1]
command = "echo from editor"
path = "/tmp"
priority = 0
dependencies = []
"#;

    let mut envs = HashMap::new();
    let editor_cmd = format!("echo '{}' > $PUEUE_EDIT_PATH ||", toml_content);
    envs.insert("EDITOR", editor_cmd.as_str());

    // Run import with the mock editor
    run_client_command_with_env(shared, &["import"], envs)?.success()?;

    // Verify the task was created
    let state = get_state(shared).await?;
    assert_eq!(state.tasks.len(), 1, "Should have imported 1 task");

    let task = state.tasks.get(&0).unwrap();
    assert_eq!(task.original_command, "echo from editor");

    Ok(())
}

/// Test that invalid TOML is rejected.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn import_invalid_toml() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Create a file with invalid TOML
    let invalid_toml = "this is not valid [ toml ]}";
    let temp_file = daemon.tempdir.path().join("invalid.toml");
    fs::write(&temp_file, invalid_toml)?;

    // Import should fail
    let output = run_client_command(shared, &["import", "-f", temp_file.to_str().unwrap()])?;

    assert!(!output.status.success(), "Should fail on invalid TOML");

    let stderr = std::str::from_utf8(&output.stderr)?;
    assert!(
        stderr.contains("Failed to deserialize"),
        "Should show deserialization error"
    );

    Ok(())
}

/// Test roundtrip: export then import produces equivalent tasks.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn roundtrip_export_import() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Create tasks with various properties
    run_client_command(
        shared,
        &["add", "-l", "test-task", "-o", "5", "echo 'roundtrip test'"],
    )?
    .success()?;

    // Export the task
    let export_output = run_client_command(shared, &["export", "0"])?.success()?;
    let exported_toml = std::str::from_utf8(&export_output.stdout)?;

    // Save to a file
    let temp_file = daemon.tempdir.path().join("roundtrip.toml");
    fs::write(&temp_file, exported_toml)?;

    // Remove the original task
    run_client_command(shared, &["remove", "0"])?.success()?;

    // Import it back
    run_client_command(shared, &["import", "-f", temp_file.to_str().unwrap()])?.success()?;

    // Verify the task has the same properties (task id will be 0 again since we removed it)
    let state = get_state(shared).await?;
    let task = state.tasks.get(&0).unwrap();
    assert_eq!(task.original_command, "echo 'roundtrip test'");
    assert_eq!(task.label, Some("test-task".to_string()));
    assert_eq!(task.priority, 5);

    Ok(())
}

/// Test importing multiple tasks maintains the order and properties.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn import_multiple_tasks() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Create a TOML file with multiple tasks
    let toml_content = r#"
[task1]
command = "echo 'first'"
path = "/tmp"
label = "first-task"
priority = 10
dependencies = []

[task2]
command = "echo 'second'"
path = "/tmp"
label = "second-task"
priority = 5
dependencies = []

[task3]
command = "echo 'third'"
path = "/tmp"
priority = 0
dependencies = []
"#;

    let temp_file = daemon.tempdir.path().join("multiple.toml");
    fs::write(&temp_file, toml_content)?;

    // Import the tasks
    run_client_command(shared, &["import", "-f", temp_file.to_str().unwrap()])?.success()?;

    // Verify all tasks were created with correct properties
    let state = get_state(shared).await?;
    assert_eq!(state.tasks.len(), 3, "Should have imported 3 tasks");

    let task0 = state.tasks.get(&0).unwrap();
    assert_eq!(task0.original_command, "echo 'first'");
    assert_eq!(task0.label, Some("first-task".to_string()));
    assert_eq!(task0.priority, 10);

    let task1 = state.tasks.get(&1).unwrap();
    assert_eq!(task1.original_command, "echo 'second'");
    assert_eq!(task1.label, Some("second-task".to_string()));
    assert_eq!(task1.priority, 5);

    let task2 = state.tasks.get(&2).unwrap();
    assert_eq!(task2.original_command, "echo 'third'");
    assert_eq!(task2.priority, 0);

    Ok(())
}

/// Test that id field in TOML is ignored and new IDs are assigned.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn import_ignores_id_field() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Create a TOML file with id fields that should be ignored
    let toml_content = r#"
[task1]
id = 999
command = "echo 'first'"
path = "/tmp"
priority = 0
dependencies = []

[task2]
id = 888
command = "echo 'second'"
path = "/tmp"
priority = 0
dependencies = []
"#;

    let temp_file = daemon.tempdir.path().join("with_ids.toml");
    fs::write(&temp_file, toml_content)?;

    // Import the tasks
    run_client_command(shared, &["import", "-f", temp_file.to_str().unwrap()])?.success()?;

    // Verify the tasks were created with auto-assigned IDs (0 and 1), not 999 and 888
    let state = get_state(shared).await?;
    assert_eq!(state.tasks.len(), 2, "Should have imported 2 tasks");

    // Tasks should have IDs 0 and 1, not the ones specified in TOML
    assert!(state.tasks.contains_key(&0), "Should have task with ID 0");
    assert!(state.tasks.contains_key(&1), "Should have task with ID 1");
    assert!(
        !state.tasks.contains_key(&999),
        "Should not have task with ID 999 from TOML"
    );
    assert!(
        !state.tasks.contains_key(&888),
        "Should not have task with ID 888 from TOML"
    );

    // Verify the commands are correct
    let task0 = state.tasks.get(&0).unwrap();
    assert_eq!(task0.original_command, "echo 'first'");

    let task1 = state.tasks.get(&1).unwrap();
    assert_eq!(task1.original_command, "echo 'second'");

    Ok(())
}
