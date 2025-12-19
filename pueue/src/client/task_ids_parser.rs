//! This module contains the logic for parsing task IDs from command line arguments.
//!
//! It supports two formats:
//! 1. Comma-separated IDs: "1,2,3" -> [1, 2, 3]
//! 2. Range notation: "start:end" or "start:end:step"
//!    - "1:5" -> [1, 2, 3, 4, 5] (inclusive on both ends)
//!    - "1:5:2" -> [1, 3, 5] (with step)
//!
//! These can be combined: "1,2:5,9" -> [1, 2, 3, 4, 5, 9]

use color_eyre::eyre::{Result, bail};
use std::collections::BTreeSet;

/// Parse a task IDs string into a sorted vector of unique task IDs.
///
/// # Format
/// - Comma-separated: "1,2,3"
/// - Range: "start:end" or "start:end:step"
/// - Combined: "1,2:5,9" -> [1, 2, 3, 4, 5, 9]
///
/// # Examples
/// ```
/// use pueue::client::task_ids_parser::parse_task_ids;
///
/// assert_eq!(parse_task_ids("1,2,3").unwrap(), vec![1, 2, 3]);
/// assert_eq!(parse_task_ids("1:3").unwrap(), vec![1, 2, 3]);
/// assert_eq!(parse_task_ids("1:5:2").unwrap(), vec![1, 3, 5]);
/// assert_eq!(parse_task_ids("1,2:5,9").unwrap(), vec![1, 2, 3, 4, 5, 9]);
/// ```
pub fn parse_task_ids(input: &str) -> Result<Vec<usize>> {
    if input.trim().is_empty() {
        return Ok(Vec::new());
    }

    let mut ids = BTreeSet::new();

    // Split by comma and process each part
    for part in input.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        // Check if it's a range (contains ':')
        if part.contains(':') {
            let range_ids = parse_range(part)
                .map_err(|e| color_eyre::eyre::eyre!("Failed to parse range '{}': {}", part, e))?;
            ids.extend(range_ids);
        } else {
            // Parse as a single ID
            let id = part.parse::<usize>().map_err(|e| {
                color_eyre::eyre::eyre!("Failed to parse task ID '{}': {}", part, e)
            })?;
            ids.insert(id);
        }
    }

    Ok(ids.into_iter().collect())
}

/// Parse a range notation like "1:5" or "1:5:2" into a vector of task IDs.
fn parse_range(range_str: &str) -> Result<Vec<usize>> {
    let parts: Vec<&str> = range_str.split(':').collect();

    match parts.len() {
        2 => {
            // Format: start:end (step = 1)
            let start = parts[0].trim().parse::<usize>().map_err(|e| {
                color_eyre::eyre::eyre!("Failed to parse start value '{}': {}", parts[0], e)
            })?;
            let end = parts[1].trim().parse::<usize>().map_err(|e| {
                color_eyre::eyre::eyre!("Failed to parse end value '{}': {}", parts[1], e)
            })?;

            if start > end {
                bail!(
                    "Range start ({}) must be less than or equal to end ({})",
                    start,
                    end
                );
            }

            Ok((start..=end).collect())
        }
        3 => {
            // Format: start:end:step
            let start = parts[0].trim().parse::<usize>().map_err(|e| {
                color_eyre::eyre::eyre!("Failed to parse start value '{}': {}", parts[0], e)
            })?;
            let end = parts[1].trim().parse::<usize>().map_err(|e| {
                color_eyre::eyre::eyre!("Failed to parse end value '{}': {}", parts[1], e)
            })?;
            let step = parts[2].trim().parse::<usize>().map_err(|e| {
                color_eyre::eyre::eyre!("Failed to parse step value '{}': {}", parts[2], e)
            })?;

            if step == 0 {
                bail!("Step must be greater than 0");
            }

            if start > end {
                bail!(
                    "Range start ({}) must be less than or equal to end ({})",
                    start,
                    end
                );
            }

            let mut ids = Vec::new();
            let mut current = start;
            while current <= end {
                ids.push(current);
                current += step;
            }

            Ok(ids)
        }
        _ => {
            bail!(
                "Invalid range format '{}'. Expected 'start:end' or 'start:end:step'",
                range_str
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_id() {
        assert_eq!(parse_task_ids("1").unwrap(), vec![1]);
        assert_eq!(parse_task_ids("42").unwrap(), vec![42]);
    }

    #[test]
    fn test_parse_comma_separated() {
        assert_eq!(parse_task_ids("1,2,3").unwrap(), vec![1, 2, 3]);
        assert_eq!(parse_task_ids("1, 2, 3").unwrap(), vec![1, 2, 3]);
        assert_eq!(parse_task_ids("3,1,2").unwrap(), vec![1, 2, 3]); // Should be sorted
    }

    #[test]
    fn test_parse_range_default_step() {
        assert_eq!(parse_task_ids("1:3").unwrap(), vec![1, 2, 3]);
        assert_eq!(parse_task_ids("1:5").unwrap(), vec![1, 2, 3, 4, 5]);
        assert_eq!(parse_task_ids("10:12").unwrap(), vec![10, 11, 12]);
    }

    #[test]
    fn test_parse_range_with_step() {
        assert_eq!(parse_task_ids("1:5:2").unwrap(), vec![1, 3, 5]);
        assert_eq!(parse_task_ids("0:10:3").unwrap(), vec![0, 3, 6, 9]);
        assert_eq!(parse_task_ids("2:8:2").unwrap(), vec![2, 4, 6, 8]);
    }

    #[test]
    fn test_parse_mixed() {
        assert_eq!(parse_task_ids("1,2:5,9").unwrap(), vec![1, 2, 3, 4, 5, 9]);
        assert_eq!(parse_task_ids("1,2:5:2,9").unwrap(), vec![1, 2, 4, 9]);
        assert_eq!(parse_task_ids("10,1:3,8").unwrap(), vec![1, 2, 3, 8, 10]);
    }

    #[test]
    fn test_parse_duplicates() {
        // Duplicates should be removed
        assert_eq!(parse_task_ids("1,2,2,3").unwrap(), vec![1, 2, 3]);
        assert_eq!(parse_task_ids("1,1:3").unwrap(), vec![1, 2, 3]);
        assert_eq!(parse_task_ids("1:3,2:4").unwrap(), vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_parse_empty() {
        assert_eq!(parse_task_ids("").unwrap(), Vec::<usize>::new());
        assert_eq!(parse_task_ids("  ").unwrap(), Vec::<usize>::new());
    }

    #[test]
    fn test_parse_single_element_range() {
        assert_eq!(parse_task_ids("5:5").unwrap(), vec![5]);
    }

    #[test]
    fn test_parse_whitespace() {
        assert_eq!(parse_task_ids(" 1, 2 , 3 ").unwrap(), vec![1, 2, 3]);
        assert_eq!(parse_task_ids(" 1:3 ").unwrap(), vec![1, 2, 3]);
        assert_eq!(parse_task_ids(" 1 : 5 : 2 ").unwrap(), vec![1, 3, 5]);
    }

    #[test]
    fn test_parse_invalid_id() {
        assert!(parse_task_ids("abc").is_err());
        assert!(parse_task_ids("1,abc,3").is_err());
    }

    #[test]
    fn test_parse_invalid_range() {
        assert!(parse_task_ids("5:1").is_err()); // start > end
        assert!(parse_task_ids("1:5:0").is_err()); // step = 0
        assert!(parse_task_ids("1:").is_err()); // incomplete range
        assert!(parse_task_ids(":5").is_err()); // incomplete range
        assert!(parse_task_ids("1:5:2:3").is_err()); // too many parts
    }

    #[test]
    fn test_parse_negative_not_supported() {
        // usize doesn't support negative numbers
        assert!(parse_task_ids("-1").is_err());
        assert!(parse_task_ids("1:-5").is_err());
    }

    #[test]
    fn test_complex_combinations() {
        assert_eq!(
            parse_task_ids("1,3:6,2,10:12:2,15").unwrap(),
            vec![1, 2, 3, 4, 5, 6, 10, 12, 15]
        );
    }
}
