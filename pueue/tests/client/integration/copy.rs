use pueue_lib::{Task, TaskStatus};

use crate::{client::helper::*, internal_prelude::*};

/// Test that copying a task creates a new task with the same command and adds the source label.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn copy_simple_task() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Create a task and wait for it to finish.
    assert_success(add_task(shared, "echo 'test'").await?);
    wait_for_task_condition(shared, 0, Task::is_done).await?;

    // Copy the task.
    run_client_command(shared, &["copy", "0"])?.success()?;

    // Check that the new task has been created.
    let state = get_state(shared).await?;

    // Should have two tasks now
    assert_eq!(state.tasks.len(), 2, "Should have two tasks after copy");

    // Check the original task (id 0)
    let original_task = state.tasks.get(&0).unwrap();
    assert_eq!(original_task.original_command, "echo 'test'");

    // Check the copied task (id 1)
    let copied_task = state.tasks.get(&1).unwrap();
    assert_eq!(copied_task.original_command, "echo 'test'");
    assert_eq!(copied_task.label, Some("(copy from #0)".to_string()));
    assert!(
        matches!(copied_task.status, TaskStatus::Stashed { .. }),
        "Copied task should be stashed by default"
    );

    Ok(())
}

/// Test that copying a task with a label preserves the label and adds the source indicator.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn copy_task_with_label() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Create a task with a label and wait for it to finish.
    run_client_command(shared, &["add", "-l", "my-task", "echo", "test"])?.success()?;
    wait_for_task_condition(shared, 0, Task::is_done).await?;

    // Copy the task.
    run_client_command(shared, &["copy", "0"])?.success()?;

    // Check that the new task has the correct label.
    let state = get_state(shared).await?;
    let copied_task = state.tasks.get(&1).unwrap();
    assert_eq!(
        copied_task.label,
        Some("my-task (copy from #0)".to_string())
    );

    Ok(())
}

/// Test that copying works on tasks in any state, not just finished ones.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn copy_running_task() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Create a long-running task.
    assert_success(add_task(shared, "sleep 60").await?);
    wait_for_task_condition(shared, 0, Task::is_running).await?;

    // Copy the running task.
    run_client_command(shared, &["copy", "0"])?.success()?;

    // Check that the new task has been created.
    let state = get_state(shared).await?;
    assert_eq!(state.tasks.len(), 2, "Should have two tasks after copy");

    let original_task = state.tasks.get(&0).unwrap();
    let copied_task = state.tasks.get(&1).unwrap();

    assert!(
        matches!(original_task.status, TaskStatus::Running { .. }),
        "Original task should still be running"
    );

    assert_eq!(copied_task.original_command, "sleep 60");
    assert!(
        matches!(copied_task.status, TaskStatus::Stashed { .. }),
        "Copied task should be stashed by default"
    );

    Ok(())
}

/// Test that copying a stashed task works correctly.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn copy_stashed_task() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Create a stashed task.
    run_client_command(shared, &["add", "-s", "echo", "test"])?.success()?;

    // Verify the task is stashed.
    let state = get_state(shared).await?;
    let original_task = state.tasks.get(&0).unwrap();
    assert!(
        matches!(original_task.status, TaskStatus::Stashed { .. }),
        "Original task should be stashed"
    );

    // Copy the stashed task.
    run_client_command(shared, &["copy", "0"])?.success()?;

    // Check that the copied task is stashed by default.
    let state = get_state(shared).await?;
    let copied_task = state.tasks.get(&1).unwrap();
    assert_eq!(copied_task.original_command, "echo test");
    assert!(
        matches!(copied_task.status, TaskStatus::Stashed { .. }),
        "Copied task should be stashed by default"
    );

    Ok(())
}

/// Test that copying with the --enqueue flag creates a queued task.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn copy_with_enqueue_flag() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Create a task and wait for it to finish.
    assert_success(add_task(shared, "echo 'test'").await?);
    wait_for_task_condition(shared, 0, Task::is_done).await?;

    // Copy the task with --enqueue flag.
    run_client_command(shared, &["copy", "--enqueue", "0"])?.success()?;

    // Check that the copied task is queued.
    let state = get_state(shared).await?;
    let copied_task = state.tasks.get(&1).unwrap();
    assert!(
        matches!(copied_task.status, TaskStatus::Queued { .. }),
        "Copied task should be queued when using --enqueue flag"
    );

    Ok(())
}

/// Test copying multiple tasks at once.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn copy_multiple_tasks() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Create multiple tasks and wait for them to finish.
    assert_success(add_task(shared, "echo 'task1'").await?);
    assert_success(add_task(shared, "echo 'task2'").await?);
    assert_success(add_task(shared, "echo 'task3'").await?);

    wait_for_task_condition(shared, 0, Task::is_done).await?;
    wait_for_task_condition(shared, 1, Task::is_done).await?;
    wait_for_task_condition(shared, 2, Task::is_done).await?;

    // Copy all three tasks.
    run_client_command(shared, &["copy", "0", "1", "2"])?.success()?;

    // Check that all three tasks have been copied.
    let state = get_state(shared).await?;
    assert_eq!(
        state.tasks.len(),
        6,
        "Should have six tasks after copying three"
    );

    // Check each copied task
    for i in 0..3 {
        let copied_task = state.tasks.get(&(i + 3)).unwrap();
        assert_eq!(copied_task.label, Some(format!("(copy from #{})", i)));
    }

    Ok(())
}
