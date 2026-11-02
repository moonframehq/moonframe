//! GitLab, over its REST API (`/api/v4`), on gitlab.com or any instance:
//! who is signed in, the project, its open merge requests and issues, and
//! the newest merge request of each branch the worktrees have out. The
//! current branch's also gets its pipeline's jobs as `check_list` and its
//! line counts, which only its diffs tell.

use {
    crate::{
        Target,
        http::{self, Failure},
        shape::{self, Avatars, Checks, Standing},
    },
    secrecy::SecretString,
    serde_json::{Map, Value, json},
};

const LISTED: usize = 30;

mod diffs;

/// `text` as one URL path segment or query value.
pub(crate) fn encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

pub fn fetch(
    agent: &ureq::Agent,
    token: &SecretString,
    target: &Target,
    avatars: &Avatars,
) -> Result<Value, Failure> {
    let api = format!("https://{}/api/v4", target.host);
    let project = format!("{api}/projects/{}", encode(&target.path));
    let get = |url: &str| http::get(agent, url, token);
    let viewer = get(&format!("{api}/user"))?;
    let repo = get(&project)?;
    let listed = get(&format!(
        "{project}/merge_requests?state=opened&order_by=updated_at&per_page={LISTED}"
    ))?;
    let issues = get(&format!(
        "{project}/issues?state=opened&order_by=updated_at&per_page={LISTED}&with_labels_details=true"
    ))
    .unwrap_or(Value::Array(Vec::new()));
    let mut by_branch = Vec::new();
    for branch in &target.branches {
        let found = get(&format!(
            "{project}/merge_requests?source_branch={}&order_by=updated_at&per_page=1",
            encode(branch)
        ));
        let Some(mr) = found
            .ok()
            .and_then(|found| found.as_array().and_then(|list| list.first().cloned()))
        else {
            continue;
        };
        let mut details = Details::default();
        if *branch == target.branch
            && let Some(iid) = mr.get("iid").and_then(Value::as_u64)
        {
            details = current(agent, token, &project, iid);
        }
        by_branch.push((branch.clone(), mr, details));
    }
    let mut people = Vec::new();
    let mut add = |person: &Value| {
        if let (Some(login), Some(url)) = (
            person.get("username").and_then(Value::as_str),
            person.get("avatar_url").and_then(Value::as_str),
        ) && !people
            .iter()
            .any(|(seen, _): &(String, String)| seen == login)
        {
            people.push((login.to_owned(), url.to_owned()));
        }
    };
    add(&viewer);
    for mr in listed.as_array().into_iter().flatten() {
        add(&mr["author"]);
    }
    avatars.fetch(agent, &people);
    Ok(shape(
        &viewer, &repo, &listed, &issues, &by_branch, target, avatars,
    ))
}

/// What only the current branch's merge request gets: its pipeline and
/// its line counts.
#[derive(Default)]
pub(crate) struct Details {
    pub(crate) mr: Option<Value>,
    pub(crate) jobs: Vec<Value>,
    pub(crate) lines: Option<(u64, u64)>,
}

fn current(agent: &ureq::Agent, token: &SecretString, project: &str, iid: u64) -> Details {
    let get = |url: &str| http::get(agent, url, token).ok();
    let mr = get(&format!("{project}/merge_requests/{iid}"));
    let jobs = mr
        .as_ref()
        .and_then(|mr| mr["head_pipeline"]["id"].as_u64())
        .and_then(|pipeline| get(&format!("{project}/pipelines/{pipeline}/jobs?per_page=50")))
        .and_then(|jobs| jobs.as_array().cloned())
        .unwrap_or_default();
    let lines = diffs::read(mr.as_ref().and_then(|mr| mr.get("changes_count")), |page| {
        get(&format!(
            "{project}/merge_requests/{iid}/diffs?per_page=100&page={page}"
        ))
    });
    Details { mr, jobs, lines }
}

pub(crate) fn shape(
    viewer: &Value,
    repo: &Value,
    listed: &Value,
    issues: &Value,
    by_branch: &[(String, Value, Details)],
    target: &Target,
    avatars: &Avatars,
) -> Value {
    let prs: Vec<Value> = listed
        .as_array()
        .into_iter()
        .flatten()
        .map(|mr| merge_request(mr, &Details::default(), avatars))
        .collect();
    let mut branches = Map::new();
    let mut current = Value::Null;
    for (branch, mr, details) in by_branch {
        let mr = merge_request(mr, details, avatars);
        if *branch == target.branch {
            current = mr.clone();
        }
        branches.insert(branch.clone(), mr);
    }
    let issues: Vec<Value> = issues
        .as_array()
        .into_iter()
        .flatten()
        .map(|issue| {
            json!({
                "number": issue["iid"],
                "title": shape::text(issue.get("title")),
                "url": shape::url(issue.get("web_url")),
                "author": person(&issue["author"], avatars),
                "updated": shape::seconds(issue.get("updated_at")),
                "labels": issue["labels"].as_array().into_iter().flatten().map(|label| json!({
                    "name": shape::text(label.get("name").or(Some(label))),
                    "color": shape::text(label.get("color")),
                })).collect::<Vec<_>>(),
                "assignees": issue["assignees"].as_array().into_iter().flatten()
                    .map(|person| person["username"].clone()).collect::<Vec<_>>(),
            })
        })
        .collect();
    json!({
        "repo": {
            "name_with_owner": shape::text(repo.get("path_with_namespace")),
            "url": shape::url(repo.get("web_url")),
            "default_branch": shape::text(repo.get("default_branch")),
        },
        "viewer": person(viewer, avatars),
        "prs": prs,
        "current_pr": current,
        "branches": branches,
        "issues": issues,
    })
}

fn person(person: &Value, avatars: &Avatars) -> Value {
    shape::person(
        &person["username"],
        &shape::text(person.get("name")),
        person["bot"] == true,
        &shape::url(person.get("avatar_url")),
        avatars,
    )
}

/// A pipeline's or job's status as a check outcome.
fn outcome(status: &str) -> &'static str {
    match status {
        "success" => "passed",
        "failed" => "failed",
        "skipped" | "canceled" | "manual" => "skipped",
        _ => "pending",
    }
}

fn merge_request(mr: &Value, details: &Details, avatars: &Avatars) -> Value {
    // The single merge request's answer has more (its pipeline); prefer it.
    let full = details.mr.as_ref().unwrap_or(mr);
    let state = match full["state"].as_str().unwrap_or_default() {
        "opened" => "open",
        "locked" => "closed",
        other => other,
    };
    let status = full["detailed_merge_status"].as_str().unwrap_or_default();
    let mut checks = Checks::default();
    if details.jobs.is_empty() {
        if let Some(pipeline) = full["head_pipeline"]["status"].as_str() {
            checks.add(outcome(pipeline));
        }
    } else {
        for job in &details.jobs {
            checks.add(outcome(job["status"].as_str().unwrap_or_default()));
        }
    }
    let standing = Standing {
        state,
        draft: full["draft"] == true || full["work_in_progress"] == true,
        conflicts: full["has_conflicts"] == true || status == "conflict",
        changes_requested: status == "requested_changes",
        review_required: status == "not_approved",
        ready: status == "mergeable",
        held: matches!(
            status,
            "blocked_status" | "discussions_not_resolved" | "need_rebase"
        ),
        checks,
    };
    let mut value = json!({
        "number": full["iid"],
        "title": shape::text(full.get("title")),
        "state": state,
        "draft": standing.draft,
        "url": shape::url(full.get("web_url")),
        "branch": shape::text(full.get("source_branch")),
        "base": shape::text(full.get("target_branch")),
        "author": person(&full["author"], avatars),
        "updated": shape::seconds(full.get("updated_at")),
        "review_decision": if standing.changes_requested { "changes_requested" } else if standing.review_required { "review_required" } else { "" },
        "mergeable": if standing.conflicts { "conflicting" } else if status == "mergeable" { "mergeable" } else { "" },
        "merge_state": status,
        "checks": checks.json(),
        "tone": standing.tone(),
    });
    if let Some((additions, deletions)) = details.lines {
        value["additions"] = json!(additions);
        value["deletions"] = json!(deletions);
    }
    if details.mr.is_some() {
        value["check_list"] = details
            .jobs
            .iter()
            .map(|job| {
                json!({
                    "name": shape::text(job.get("name")),
                    "outcome": outcome(job["status"].as_str().unwrap_or_default()),
                    "url": shape::url(job.get("web_url")),
                })
            })
            .collect();
    }
    value
}

/// Who `token` belongs to on `host`.
pub fn login(agent: &ureq::Agent, host: &str, token: &SecretString) -> Result<String, Failure> {
    let user = http::get(agent, &format!("https://{host}/api/v4/user"), token)?;
    user.get("username")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| Failure::Body("no username".into()))
}
