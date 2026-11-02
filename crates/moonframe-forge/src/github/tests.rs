use super::*;

#[test]
fn branch_lookup_skips_other_forks_and_continues_past_the_first_page() {
    let first = json!({
        "nodes": [{ "number": 9, "headRepository": { "nameWithOwner": "other/repo" } }],
        "pageInfo": { "hasNextPage": true, "endCursor": "next" },
    });
    let found = branch_request(first, "owner/repo", |cursor| {
        assert_eq!(cursor, "next");
        Ok(json!({ "nodes": [
            { "number": 8, "headRepository": { "nameWithOwner": "another/repo" } },
            { "number": 7, "headRepository": { "nameWithOwner": "OWNER/repo" } },
            { "number": 6, "headRepository": { "nameWithOwner": "owner/repo" } },
        ], "pageInfo": { "hasNextPage": false } }))
    })
    .unwrap()
    .unwrap();
    assert_eq!(found["number"], 7);
    let forks_only = json!({ "nodes": [{ "headRepository": { "nameWithOwner": "fork/repo" } }],
        "pageInfo": { "hasNextPage": false } });
    assert!(
        branch_request(forks_only, "owner/repo", |_| panic!("no further page"))
            .unwrap()
            .is_none()
    );
}

#[test]
fn checks_use_complete_counts_even_when_the_display_list_is_truncated() {
    let folder = tempfile::tempdir().unwrap();
    let avatars = Avatars {
        folder: folder.path(),
        prefix: String::new(),
    };
    let pr = json!({
        "state": "OPEN", "mergeStateStatus": "BLOCKED", "isDraft": false,
        "commits": { "nodes": [{ "commit": { "statusCheckRollup": { "contexts": {
            "nodes": vec![json!({ "conclusion": "SUCCESS", "status": "COMPLETED", "name": "test" }); 50],
            "checkRunCountsByState": [
                { "state": "SUCCESS", "count": 50 },
                { "state": "FAILURE", "count": 1 },
                { "state": "WAITING", "count": 2 },
                { "state": "NEUTRAL", "count": 3 },
            ],
            "statusContextCountsByState": [
                { "state": "ERROR", "count": 1 },
                { "state": "SUCCESS", "count": 1 },
                { "state": "EXPECTED", "count": 1 },
            ],
        } } } }] },
    });
    let out = pull_request(&pr, &avatars, true);
    assert_eq!(
        out["checks"],
        json!({ "total": 59, "passed": 51, "failed": 2, "pending": 3, "skipped": 3 })
    );
    assert_eq!(out["tone"], "attention");
    assert_eq!(out["check_list"].as_array().unwrap().len(), CHECKS);
}
