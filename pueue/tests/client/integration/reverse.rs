use crate::{client::helper::*, internal_prelude::*};

/// Test reversing an even number of tasks.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reverse_even_number_of_tasks() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Add four stashed tasks
    for i in 0..4 {
        create_stashed_task(shared, &format!("task_{}", i), None).await?;
    }

    // Reverse the tasks
    run_client_command(shared, &["reverse", "0,1,2,3"])?.success()?;

    // Verify that tasks have been reversed: [0,1,2,3] -> [3,2,1,0]
    let state = get_state(shared).await?;
    assert_eq!(state.tasks.get(&0).unwrap().command, "task_3");
    assert_eq!(state.tasks.get(&1).unwrap().command, "task_2");
    assert_eq!(state.tasks.get(&2).unwrap().command, "task_1");
    assert_eq!(state.tasks.get(&3).unwrap().command, "task_0");

    Ok(())
}

/// Test reversing an odd number of tasks.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reverse_odd_number_of_tasks() -> Result<()> {
    let daemon = daemon().await?;
    let shared = &daemon.settings.shared;

    // Add five stashed tasks
    for i in 0..5 {
        create_stashed_task(shared, &format!("task_{}", i), None).await?;
    }

    // Reverse the tasks
    run_client_command(shared, &["reverse", "0:4"])?.success()?;

    // Verify that tasks have been reversed: [0,1,2,3,4] -> [4,3,2,1,0]
    let state = get_state(shared).await?;
    assert_eq!(state.tasks.get(&0).unwrap().command, "task_4");
    assert_eq!(state.tasks.get(&1).unwrap().command, "task_3");
    assert_eq!(state.tasks.get(&2).unwrap().command, "task_2");
    assert_eq!(state.tasks.get(&3).unwrap().command, "task_1");
    assert_eq!(state.tasks.get(&4).unwrap().command, "task_0");

    Ok(())
}
