use {super::*, serde_json::json};

#[test]
fn branch_lookup_skips_forks_and_reads_the_next_page() {
    let mut requested = Vec::new();
    let result = pages::branch(7, |page| {
        requested.push(page);
        Ok(if page == 1 {
            json!(vec![json!({ "iid": 90, "source_project_id": 8 }); 100])
        } else {
            json!([
                { "iid": 80, "source_project_id": 9 },
                { "iid": 70, "source_project_id": 7 },
                { "iid": 60, "source_project_id": 7 }
            ])
        })
    })
    .unwrap()
    .unwrap();
    assert_eq!(requested, [1, 2]);
    assert_eq!(result["iid"], 70);
    assert!(
        pages::branch(7, |_| Ok(json!([{ "source_project_id": 8 }])))
            .unwrap()
            .is_none()
    );
    assert_eq!(
        pages::branch(7, |_| Err(Failure::RateLimited)),
        Err(Failure::RateLimited)
    );
}

#[test]
fn checks_include_failures_beyond_the_display_limit_and_first_page() {
    let jobs = pages::jobs(|page| {
        Ok(if page == 1 {
            json!(vec![json!({ "name": "test", "status": "success" }); 100])
        } else {
            json!([{ "name": "early-failure", "status": "failed" }])
        })
    })
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let avatars = Avatars {
        folder: dir.path(),
        prefix: String::new(),
    };
    let mr = json!({ "state": "opened", "head_pipeline": { "status": "failed" } });
    let result = merge_request(
        &mr,
        &Details {
            mr: Some(mr.clone()),
            jobs,
            lines: None,
        },
        &avatars,
    );
    assert_eq!(result["checks"]["total"], 101);
    assert_eq!(result["checks"]["failed"], 1);
    assert_eq!(result["tone"], "attention");
    assert_eq!(result["check_list"].as_array().unwrap().len(), 50);
}

#[test]
fn a_failed_job_page_never_publishes_partial_totals() {
    for failure in [
        Failure::Network("offline".into()),
        Failure::RateLimited,
        Failure::Unauthorized,
    ] {
        let result = pages::jobs(|page| {
            if page == 1 {
                Ok(json!(vec![json!({ "status": "success" }); 100]))
            } else {
                Err(failure.clone())
            }
        });
        assert_eq!(result, Err(failure));
    }
}

#[test]
fn a_failed_pipeline_is_not_hidden_by_successful_jobs() {
    let dir = tempfile::tempdir().unwrap();
    let avatars = Avatars {
        folder: dir.path(),
        prefix: String::new(),
    };
    for (status, counter) in [("failed", "failed"), ("running", "pending")] {
        let mr = json!({ "state": "opened", "head_pipeline": { "status": status } });
        let result = merge_request(
            &mr,
            &Details {
                mr: Some(mr.clone()),
                jobs: vec![json!({ "status": "success" })],
                lines: None,
            },
            &avatars,
        );
        assert_eq!(result["checks"][counter], 1);
        assert_eq!(result["checks"]["passed"], 0);
    }
}

#[test]
fn inaccessible_jobs_fall_back_to_the_pipeline_without_partial_counts() {
    let dir = tempfile::tempdir().unwrap();
    let avatars = Avatars {
        folder: dir.path(),
        prefix: String::new(),
    };
    for status in [403, 404] {
        let jobs = pages::jobs(|page| {
            if page == 1 {
                Ok(json!(vec![json!({ "status": "success" }); 100]))
            } else {
                Err(Failure::Status(status, String::new()))
            }
        })
        .unwrap();
        assert!(jobs.is_empty());
        let mr = json!({ "state": "opened", "head_pipeline": { "status": "failed" } });
        let result = merge_request(
            &mr,
            &Details {
                mr: Some(mr.clone()),
                jobs,
                lines: None,
            },
            &avatars,
        );
        assert_eq!(result["checks"]["total"], 1);
        assert_eq!(result["checks"]["failed"], 1);
        assert_eq!(result["tone"], "attention");
    }
}
