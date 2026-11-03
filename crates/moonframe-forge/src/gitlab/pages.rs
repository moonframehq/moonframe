//! Bounded pagination: branch names are scoped to their source project;
//! job totals cover every page, even though the UI lists only fifty jobs.

use {crate::http::Failure, serde_json::Value};

pub(super) fn branch(
    project: u64,
    mut get: impl FnMut(usize) -> Result<Value, Failure>,
) -> Result<Option<Value>, Failure> {
    for page in 1..=100 {
        let value = get(page)?;
        let list = array(&value)?;
        if let Some(mr) = list
            .iter()
            .find(|mr| mr["source_project_id"].as_u64() == Some(project))
        {
            return Ok(Some(mr.clone()));
        }
        if list.len() < 100 {
            return Ok(None);
        }
    }
    Err(Failure::Body(
        "too many merge-request pages for this branch".into(),
    ))
}

pub(super) fn jobs(
    mut get: impl FnMut(usize) -> Result<Value, Failure>,
) -> Result<Vec<Value>, Failure> {
    let mut jobs = Vec::new();
    for page in 1..=100 {
        let value = match get(page) {
            Ok(value) => value,
            // Job visibility can differ from the MR's. Discard every
            // page and let the caller use the head pipeline's status.
            Err(Failure::Status(403 | 404, _)) => return Ok(Vec::new()),
            Err(failure) => return Err(failure),
        };
        let list = array(&value)?;
        jobs.extend(list.iter().cloned());
        if list.len() < 100 {
            return Ok(jobs);
        }
    }
    Err(Failure::Body("too many pipeline job pages".into()))
}

fn array(value: &Value) -> Result<&Vec<Value>, Failure> {
    value
        .as_array()
        .ok_or_else(|| Failure::Body("the page is not a list".into()))
}
