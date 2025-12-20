use pueue_lib::task::TaskStatus;

use crate::{client::helper::*, internal_prelude::*};

/// Test that exporting tasks produces valid TOML output.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn export_simple_task() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Create a task
    assert_success(add_task(shared, "echo 'test'").await?);
    wait_for_task_condition(shared, 0, |task| {
        matches!(task.status, TaskStatus::Done { .. })
    })
    .await?;

    // Export the task
    let output = run_client_command(shared, &["export", "0"])?.success()?;

    // Check that the output contains valid TOML with the task
    let output_str = std::str::from_utf8(&output.stdout)?;
    assert!(output_str.contains("[0]"), "Should contain task section");
    assert!(
        output_str.contains("command = \"echo 'test'\""),
        "Should contain the command"
    );

    // Verify it's valid TOML
    let parsed: toml::Value = toml::from_str(output_str)?;
    assert!(parsed.get("0").is_some(), "Should have task 0");

    Ok(())
}

/// Test that exporting multiple tasks with natural sorting.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn export_multiple_tasks_natural_order() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Create tasks with IDs that will test natural sorting (2, 10, 3)
    for i in 0..12 {
        let command = format!("echo 'task {}'", i);
        assert_success(add_task(shared, &command).await?);
    }

    // Export all tasks
    let output = run_client_command(shared, &["export"])?.success()?;
    let output_str = std::str::from_utf8(&output.stdout)?;

    // Parse the TOML to verify all tasks are present
    let parsed: toml::Value = toml::from_str(output_str)?;

    // Verify all 12 tasks are exported
    for i in 0..12 {
        let task_key = i.to_string();
        assert!(parsed.get(&task_key).is_some(), "Should have task {}", i);
    }

    // Check that task sections appear in natural order by looking at line positions
    // The TOML sections should be in the order: [0], [1], [2], ..., [9], [10], [11]
    let lines: Vec<&str> = output_str.lines().collect();
    let mut last_id: Option<usize> = None;

    for line in lines {
        if let Some(stripped) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            if let Ok(id) = stripped.parse::<usize>() {
                if let Some(prev_id) = last_id {
                    assert!(
                        id > prev_id,
                        "Task {} should appear after task {} (natural order)",
                        id,
                        prev_id
                    );
                }
                last_id = Some(id);
            }
        }
    }

    Ok(())
}

/// Test exporting tasks from a specific group.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn export_specific_group() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Create a new group
    run_client_command(shared, &["group", "add", "testgroup"])?.success()?;

    // Add tasks to different groups
    run_client_command(shared, &["add", "echo 'default'"])?.success()?;
    run_client_command(shared, &["add", "-g", "testgroup", "echo 'test'"])?.success()?;

    // Export only the testgroup
    let output = run_client_command(shared, &["export", "-g", "testgroup"])?.success()?;
    let output_str = std::str::from_utf8(&output.stdout)?;

    // Should contain the testgroup task
    assert!(
        output_str.contains("echo 'test'"),
        "Should contain testgroup task"
    );
    // Should not contain the default group task
    assert!(
        !output_str.contains("echo 'default'"),
        "Should not contain default group task"
    );

    Ok(())
}

/// Test exporting specific task IDs.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn export_specific_tasks() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Create multiple tasks
    for i in 0..5 {
        let command = format!("echo 'task {}'", i);
        assert_success(add_task(shared, &command).await?);
    }

    // Export only tasks 1 and 3
    let output = run_client_command(shared, &["export", "1,3"])?.success()?;
    let output_str = std::str::from_utf8(&output.stdout)?;

    // Should contain tasks 1 and 3
    assert!(output_str.contains("[1]"), "Should contain task 1");
    assert!(output_str.contains("[3]"), "Should contain task 3");

    // Should not contain other tasks
    assert!(!output_str.contains("[0]"), "Should not contain task 0");
    assert!(!output_str.contains("[2]"), "Should not contain task 2");
    assert!(!output_str.contains("[4]"), "Should not contain task 4");

    Ok(())
}

/// Test exporting tasks with all properties (label, priority, dependencies).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn export_task_with_properties() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Create a task with dependencies
    assert_success(add_task(shared, "echo 'first'").await?);

    // Create a task with label, priority, and dependency
    run_client_command(
        shared,
        &[
            "add",
            "-l",
            "important",
            "-o",
            "10",
            "--after",
            "0",
            "--",
            "echo",
            "second",
        ],
    )?
    .success()?;

    // Export task 1
    let output = run_client_command(shared, &["export", "1"])?.success()?;
    let output_str = std::str::from_utf8(&output.stdout)?;

    // Verify all properties are exported
    assert!(
        output_str.contains("label = \"important\""),
        "Should export label"
    );
    assert!(
        output_str.contains("priority = 10"),
        "Should export priority"
    );
    assert!(
        output_str.contains("dependencies = [0]"),
        "Should export dependencies"
    );

    Ok(())
}

/// Test exporting when no tasks exist or match the criteria.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn export_no_tasks() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Try to export when no tasks exist
    let output = run_client_command(shared, &["export"])?;

    // Should fail with an error message
    assert!(!output.status.success(), "Should fail when no tasks exist");

    let stderr = std::str::from_utf8(&output.stderr)?;
    assert!(
        stderr.contains("No tasks found"),
        "Should show 'No tasks found' error message"
    );

    Ok(())
}

/// Test exporting tasks with special characters in commands.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn export_task_with_special_characters() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Create a task with special characters
    run_client_command(shared, &["add", "echo 'test \"quoted\" && echo $VAR'"])?.success()?;

    // Export the task
    let output = run_client_command(shared, &["export", "0"])?.success()?;
    let output_str = std::str::from_utf8(&output.stdout)?;

    // Verify the command is properly escaped in TOML
    let parsed: toml::Value = toml::from_str(output_str)?;
    let task = parsed.get("0").expect("Should have task 0");
    let command = task
        .get("command")
        .and_then(|v| v.as_str())
        .expect("Should have command field");

    assert!(
        command.contains("quoted"),
        "Should preserve special characters"
    );

    Ok(())
}
