use pueue_lib::{Task, TaskStatus};

use crate::{client::helper::*, internal_prelude::*};

/// The name of the special archive group.
/// This is used internally by the archive functionality.
const PUEUE_ARCHIVE_GROUP: &str = "archive";

/// Test that archiving a finished task works correctly.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn archive_finished_task() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Create a task and wait for it to finish.
    assert_success(add_task(shared, "echo 'test'").await?);
    wait_for_task_condition(shared, 0, Task::is_done).await?;

    // Archive the task.
    run_client_command(shared, &["archive", "0"])?.success()?;

    // Check that the original task has been removed and a new one created in archive group.
    let state = get_state(shared).await?;

    // Original task should be gone
    assert!(
        !state.tasks.contains_key(&0),
        "Original task should be removed"
    );

    // Should have one task in archive group
    let archived_tasks: Vec<_> = state
        .tasks
        .values()
        .filter(|t| t.group == PUEUE_ARCHIVE_GROUP)
        .collect();
    assert_eq!(
        archived_tasks.len(),
        1,
        "Should have one task in archive group"
    );

    let archived_task = archived_tasks[0];
    assert_eq!(archived_task.original_command, "echo 'test'");
    assert!(
        matches!(archived_task.status, TaskStatus::Stashed { .. }),
        "Archived task should be stashed"
    );

    Ok(())
}

/// Test that archiving a task with a label preserves the label.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn archive_task_with_label() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Create a task with a label and wait for it to finish.
    run_client_command(shared, &["add", "-l", "important", "echo", "test"])?.success()?;
    wait_for_task_condition(shared, 0, Task::is_done).await?;

    // Archive the task.
    run_client_command(shared, &["archive", "0"])?.success()?;

    // Check that the archived task has the correct label.
    let state = get_state(shared).await?;
    let archived_task = state
        .tasks
        .values()
        .find(|t| t.group == PUEUE_ARCHIVE_GROUP)
        .expect("Should have archived task");

    assert_eq!(archived_task.label, Some("important".to_string()));

    Ok(())
}

/// Test that archiving multiple tasks works correctly.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn archive_multiple_tasks() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Create three tasks and wait for them to finish.
    assert_success(add_task(shared, "echo 'task1'").await?);
    assert_success(add_task(shared, "echo 'task2'").await?);
    assert_success(add_task(shared, "echo 'task3'").await?);
    wait_for_task_condition(shared, 0, Task::is_done).await?;
    wait_for_task_condition(shared, 1, Task::is_done).await?;
    wait_for_task_condition(shared, 2, Task::is_done).await?;

    // Archive all three tasks.
    run_client_command(shared, &["archive", "0,1,2"])?.success()?;

    // Check that all original tasks are removed.
    let state = get_state(shared).await?;
    assert!(!state.tasks.contains_key(&0));
    assert!(!state.tasks.contains_key(&1));
    assert!(!state.tasks.contains_key(&2));

    // Should have three tasks in archive group
    let archived_tasks: Vec<_> = state
        .tasks
        .values()
        .filter(|t| t.group == PUEUE_ARCHIVE_GROUP)
        .collect();
    assert_eq!(archived_tasks.len(), 3, "Should have three archived tasks");

    Ok(())
}

/// Test that archiving a running task fails.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn archive_running_task_fails() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Create a long-running task.
    assert_success(add_task(shared, "sleep 60").await?);
    wait_for_task_condition(shared, 0, Task::is_running).await?;

    // Attempt to archive the running task should fail or skip it.
    let result = run_client_command(shared, &["archive", "0"]);
    assert!(
        result.is_err() || !result.unwrap().status.success(),
        "Archiving a running task should fail or indicate it was skipped"
    );

    // Original task should still be running
    let state = get_state(shared).await?;
    let task = state.tasks.get(&0).expect("Task should still exist");
    assert!(
        matches!(task.status, TaskStatus::Running { .. }),
        "Task should still be running"
    );

    Ok(())
}

/// Test that archiving a queued task works.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn archive_queued_task() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Pause the default group and create a task.
    run_client_command(shared, &["pause"])?.success()?;
    assert_success(add_task(shared, "echo 'test'").await?);

    // Archive the queued task.
    run_client_command(shared, &["archive", "0"])?.success()?;

    // Check that the task was archived.
    let state = get_state(shared).await?;
    assert!(
        !state.tasks.contains_key(&0),
        "Original task should be removed"
    );

    let archived_tasks: Vec<_> = state
        .tasks
        .values()
        .filter(|t| t.group == PUEUE_ARCHIVE_GROUP)
        .collect();
    assert_eq!(archived_tasks.len(), 1, "Should have one archived task");

    Ok(())
}

/// Test that archiving a stashed task works.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn archive_stashed_task() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Create a stashed task.
    run_client_command(shared, &["add", "-s", "echo", "test"])?.success()?;

    // Archive the stashed task.
    run_client_command(shared, &["archive", "0"])?.success()?;

    // Check that the task was archived.
    let state = get_state(shared).await?;
    let archived_task = state
        .tasks
        .values()
        .find(|t| t.group == PUEUE_ARCHIVE_GROUP)
        .expect("Should have archived task");
    assert_eq!(archived_task.original_command, "echo test");

    Ok(())
}

/// Test that viewing the archive group without any archived tasks creates the group.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn view_empty_archive() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // First archive a task to create the archive group
    assert_success(add_task(shared, "echo 'temp'").await?);
    wait_for_task_condition(shared, 0, Task::is_done).await?;
    run_client_command(shared, &["archive", "0"])?.success()?;

    // Now remove the archived task from the archive group
    // (assuming we can access it via status -g archive and remove it)
    let state = get_state(shared).await?;
    let archived_id = state
        .tasks
        .values()
        .find(|t| t.group == PUEUE_ARCHIVE_GROUP)
        .map(|t| t.id)
        .expect("Should have archived task");

    run_client_command(shared, &["remove", &archived_id.to_string()])?.success()?;

    // Now view the empty archive group
    let output = run_client_command(shared, &["archive"])?.success()?;
    let stdout = String::from_utf8(output.stdout)?;

    // Should show archive group header even when empty
    assert!(
        stdout.contains("archive") || stdout.contains("empty"),
        "Should show archive group or empty message"
    );

    Ok(())
}

/// Test that viewing the archive group shows archived tasks.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn view_archive_with_tasks() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Create and archive a task.
    assert_success(add_task(shared, "echo 'archived'").await?);
    wait_for_task_condition(shared, 0, Task::is_done).await?;
    run_client_command(shared, &["archive", "0"])?.success()?;

    // View the archive group.
    let output = run_client_command(shared, &["archive"])?.success()?;
    let stdout = String::from_utf8(output.stdout)?;

    // Should show the archived task
    assert!(
        stdout.contains("archived") || stdout.contains("echo"),
        "Should show archived task"
    );

    Ok(())
}

/// Test that archived tasks are hidden from normal status view.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn archived_tasks_hidden_from_status() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Create two tasks and archive one.
    assert_success(add_task(shared, "echo 'first'").await?);
    assert_success(add_task(shared, "echo 'second'").await?);
    wait_for_task_condition(shared, 0, Task::is_done).await?;
    wait_for_task_condition(shared, 1, Task::is_done).await?;

    run_client_command(shared, &["archive", "0"])?.success()?;

    // Get normal status - should only show task 1
    let output = run_client_command(shared, &["status"])?.success()?;
    let stdout = String::from_utf8(output.stdout)?;

    // Should show task 1 but not task 0
    assert!(
        stdout.contains("second") || stdout.contains("1"),
        "Should show non-archived task"
    );
    // Archive group should not be visible in normal status
    assert!(
        !stdout.contains("archive"),
        "Archive group should be hidden from normal status"
    );

    Ok(())
}

/// Test that the archive group is visible in group listing.
/// Unlike status, the group command should show all groups including archive.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn archive_group_visible_in_group_list() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Create and archive a task to ensure archive group exists.
    assert_success(add_task(shared, "echo 'test'").await?);
    wait_for_task_condition(shared, 0, Task::is_done).await?;
    run_client_command(shared, &["archive", "0"])?.success()?;

    // Verify archive group exists in daemon state
    let state = get_state(shared).await?;
    assert!(
        state.groups.contains_key(PUEUE_ARCHIVE_GROUP),
        "Archive group should exist in daemon state"
    );

    // List groups - archive should be visible here
    let output = run_client_command(shared, &["group"])?.success()?;
    let stdout = String::from_utf8(output.stdout)?;

    // Both default and archive groups should be visible
    assert!(
        stdout.contains("default"),
        "Default group should be visible"
    );

    assert!(
        stdout.contains("archive"),
        "Archive group should be visible in group list. Output:\n{}",
        stdout
    );

    Ok(())
}

/// Test that trying to archive a non-existent task reports an error.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn archive_nonexistent_task() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Try to archive a task that doesn't exist.
    let result = run_client_command(shared, &["archive", "999"]);
    assert!(
        result.is_err() || !result.unwrap().status.success(),
        "Archiving non-existent task should fail"
    );

    Ok(())
}

/// Test that archiving preserves task priority.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn archive_preserves_priority() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Create a task with custom priority.
    run_client_command(shared, &["add", "-o", "5", "echo", "test"])?.success()?;
    wait_for_task_condition(shared, 0, Task::is_done).await?;

    // Archive the task.
    run_client_command(shared, &["archive", "0"])?.success()?;

    // Check that priority is preserved.
    let state = get_state(shared).await?;
    let archived_task = state
        .tasks
        .values()
        .find(|t| t.group == PUEUE_ARCHIVE_GROUP)
        .expect("Should have archived task");
    assert_eq!(archived_task.priority, 5, "Priority should be preserved");

    Ok(())
}

/// Test that tasks with active dependencies cannot be archived.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn archive_task_with_active_dependencies() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Create a finished task and a queued task that depends on it.
    assert_success(add_task(shared, "echo 'first'").await?);
    wait_for_task_condition(shared, 0, Task::is_done).await?;

    run_client_command(shared, &["pause"])?.success()?;
    run_client_command(shared, &["add", "--after", "0", "--", "echo", "second"])?.success()?;

    // Try to archive task 0 - should fail or skip because task 1 depends on it
    let result = run_client_command(shared, &["archive", "0"]);
    // The command might succeed but should report that task 0 was skipped
    if let Ok(output) = result {
        let stderr = String::from_utf8(output.stderr)?;
        assert!(
            stderr.contains("Skipped") || stderr.contains("cannot be archived"),
            "Should indicate task with dependants cannot be archived"
        );
    }

    // Task 0 should still exist
    let state = get_state(shared).await?;
    assert!(
        state.tasks.contains_key(&0),
        "Task with active dependants should not be archived"
    );

    Ok(())
}

/// Test that archiving both a task and its dependent works.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn archive_task_and_dependent_together() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Create two finished tasks with dependency.
    assert_success(add_task(shared, "echo 'first'").await?);
    wait_for_task_condition(shared, 0, Task::is_done).await?;

    run_client_command(shared, &["add", "--after", "0", "--", "echo", "second"])?.success()?;
    run_client_command(shared, &["start", "1"])?.success()?;
    wait_for_task_condition(shared, 1, Task::is_done).await?;

    // Archive both tasks together
    run_client_command(shared, &["archive", "0,1"])?.success()?;

    // Both should be archived
    let state = get_state(shared).await?;
    assert!(!state.tasks.contains_key(&0));
    assert!(!state.tasks.contains_key(&1));

    let archived_count = state
        .tasks
        .values()
        .filter(|t| t.group == PUEUE_ARCHIVE_GROUP)
        .count();
    assert_eq!(archived_count, 2, "Both tasks should be archived");

    Ok(())
}
