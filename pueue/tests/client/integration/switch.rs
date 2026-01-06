use assert_matches::assert_matches;
use pueue_lib::{Task, TaskStatus, message::TaskSelection};

use crate::{client::helper::*, internal_prelude::*};

/// Test switching two single tasks.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn switch_two_tasks() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Add two stashed tasks
    create_stashed_task(shared, "task_0", None).await?;
    create_stashed_task(shared, "task_1", None).await?;

    // Switch the tasks
    run_client_command(shared, &["switch", "0", "1"])?.success()?;

    // Verify that tasks have been switched
    let state = get_state(shared).await?;
    let task_0 = state.tasks.get(&0).unwrap();
    let task_1 = state.tasks.get(&1).unwrap();

    assert_eq!(
        task_0.command, "task_1",
        "Task 0 should now have command from task 1"
    );
    assert_eq!(
        task_1.command, "task_0",
        "Task 1 should now have command from task 0"
    );

    Ok(())
}

/// Test switching multiple task pairs at once.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn switch_multiple_task_pairs() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Add six stashed tasks
    for i in 0..6 {
        create_stashed_task(shared, &format!("task_{}", i), None).await?;
    }

    // Switch three pairs of tasks: (0,3), (1,4), (2,5)
    let response = send_request(
        shared,
        pueue_lib::message::SwitchRequest {
            task_ids_1: vec![0, 1, 2],
            task_ids_2: vec![3, 4, 5],
        },
    )
    .await?;
    assert_success(response);

    // Verify all tasks have been switched correctly
    let state = get_state(shared).await?;

    assert_eq!(state.tasks.get(&0).unwrap().command, "task_3");
    assert_eq!(state.tasks.get(&1).unwrap().command, "task_4");
    assert_eq!(state.tasks.get(&2).unwrap().command, "task_5");
    assert_eq!(state.tasks.get(&3).unwrap().command, "task_0");
    assert_eq!(state.tasks.get(&4).unwrap().command, "task_1");
    assert_eq!(state.tasks.get(&5).unwrap().command, "task_2");

    Ok(())
}

/// Test that switching with mismatched list lengths fails on the client side.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn switch_mismatched_lengths_fails() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Add tasks
    for i in 0..5 {
        create_stashed_task(shared, &format!("task_{}", i), None).await?;
    }

    // Try to switch lists of different lengths - should fail during command execution
    // The client will send the parsed lists to the server
    let response = send_request(
        shared,
        pueue_lib::message::SwitchRequest {
            task_ids_1: vec![0, 1],
            task_ids_2: vec![2, 3, 4],
        },
    )
    .await?;
    assert_failure(response);

    Ok(())
}

/// Test that switching non-queued/stashed tasks fails.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn switch_running_task_fails() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Add a task that will run
    assert_success(add_task(shared, "sleep 10").await?);
    // Add a stashed task
    create_stashed_task(shared, "task_1", None).await?;

    // Wait for the first task to start running
    wait_for_task_condition(shared, 0, Task::is_running).await?;

    // Try to switch running task with stashed task - should be rejected by server
    let response = send_request(
        shared,
        pueue_lib::message::SwitchRequest {
            task_ids_1: vec![0],
            task_ids_2: vec![1],
        },
    )
    .await?;
    assert_failure(response);

    Ok(())
}

/// Test that task dependencies are updated correctly after switching.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn switch_updates_dependencies() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Add two stashed tasks
    create_stashed_task(shared, "task_0", None).await?;
    create_stashed_task(shared, "task_1", None).await?;

    // Add a third task that depends on task 0
    let mut message = create_add_message(shared, "task_2");
    message.stashed = true;
    message.dependencies = vec![0];
    send_request(shared, message).await?;

    // Switch tasks 0 and 1
    run_client_command(shared, &["switch", "0", "1"])?.success()?;

    // Verify that task 2's dependency has been updated to point to task 1
    let state = get_state(shared).await?;
    let task_2 = state.tasks.get(&2).unwrap();
    assert_eq!(
        task_2.dependencies,
        vec![1],
        "Dependencies should be updated after switch"
    );

    Ok(())
}

/// Test switching queued tasks (not just stashed).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn switch_queued_tasks() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Pause the daemon so tasks stay queued
    pause_tasks(shared, TaskSelection::All).await?;

    // Add two tasks (they will be queued but not run)
    assert_success(add_task(shared, "task_0").await?);
    assert_success(add_task(shared, "task_1").await?);

    // Verify they are queued
    let state = get_state(shared).await?;
    assert_matches!(
        state.tasks.get(&0).unwrap().status,
        TaskStatus::Queued { .. }
    );
    assert_matches!(
        state.tasks.get(&1).unwrap().status,
        TaskStatus::Queued { .. }
    );

    // Switch the tasks
    run_client_command(shared, &["switch", "0", "1"])?.success()?;

    // Verify that tasks have been switched
    let state = get_state(shared).await?;
    assert_eq!(state.tasks.get(&0).unwrap().command, "task_1");
    assert_eq!(state.tasks.get(&1).unwrap().command, "task_0");

    Ok(())
}

/// Test that dependencies on both switched tasks remain unchanged.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn switch_double_dependency_unchanged() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Add two stashed tasks
    create_stashed_task(shared, "task_0", None).await?;
    create_stashed_task(shared, "task_1", None).await?;

    // Add a third task that depends on BOTH tasks 0 and 1
    let mut message = create_add_message(shared, "task_2");
    message.stashed = true;
    message.dependencies = vec![0, 1];
    send_request(shared, message).await?;

    // Switch tasks 0 and 1
    run_client_command(shared, &["switch", "0", "1"])?.success()?;

    // Verify that task 2's dependencies are still [0, 1] (just switched places)
    let state = get_state(shared).await?;
    let task_2 = state.tasks.get(&2).unwrap();
    assert_eq!(
        task_2.dependencies,
        vec![0, 1],
        "Dependencies on both switched tasks should remain unchanged"
    );

    Ok(())
}
