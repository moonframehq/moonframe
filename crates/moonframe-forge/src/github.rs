//! GitHub, in one GraphQL request: who is signed in, the repository, its
//! open pull requests and issues, and the newest pull request of each
//! branch the checkout's worktrees have out (open, merged or closed), with
//! its checks and line counts.

use {
    crate::{
        Target,
        http::{self, Failure},
        shape::{self, Avatars, Checks, Standing},
    },
    secrecy::SecretString,
    serde_json::{Map, Value, json},
};

/// Open pull requests and issues listed.
const LISTED: usize = 30;
/// Checks read per pull request.
const CHECKS: usize = 50;

const PR_FIELDS: &str = "number title state isDraft url headRefName baseRefName updatedAt \
additions deletions changedFiles reviewDecision mergeable mergeStateStatus \
author { __typename login avatarUrl(size: 64) ... on User { name } } \
commits(last: 1) { nodes { commit { statusCheckRollup { contexts(first: 50) { nodes { \
__typename ... on CheckRun { name status conclusion detailsUrl } \
... on StatusContext { context state targetUrl } } } } } } }";

/// The query for `branches` (each an alias `b0`, `b1`, ...).
fn query(branches: usize) -> String {
    let mut variables = String::from("$owner: String!, $name: String!");
    let mut aliases = String::new();
    for index in 0..branches {
        variables.push_str(&format!(", $b{index}: String!"));
        aliases.push_str(&format!(
            " b{index}: pullRequests(headRefName: $b{index}, first: 1, \
             orderBy: {{ field: UPDATED_AT, direction: DESC }}, \
             states: [OPEN, MERGED, CLOSED]) {{ nodes {{ ...pr }} }}"
        ));
    }
    format!(
        "query({variables}) {{ viewer {{ login name avatarUrl(size: 64) }} \
         repository(owner: $owner, name: $name) {{ nameWithOwner url defaultBranchRef {{ name }} \
         pullRequests(states: OPEN, first: {LISTED}, orderBy: {{ field: UPDATED_AT, direction: DESC }}) \
         {{ nodes {{ ...pr }} }} \
         issues(states: OPEN, first: {LISTED}, orderBy: {{ field: UPDATED_AT, direction: DESC }}) \
         {{ nodes {{ number title url updatedAt \
         author {{ __typename login avatarUrl(size: 64) ... on User {{ name }} }} \
         labels(first: 10) {{ nodes {{ name color }} }} assignees(first: 10) {{ nodes {{ login }} }} }} }}\
         {aliases} }} }} fragment pr on PullRequest {{ {PR_FIELDS} }}"
    )
}

/// Everything about `target`'s repository the provider publishes.
pub fn fetch(
    agent: &ureq::Agent,
    token: &SecretString,
    target: &Target,
    avatars: &Avatars,
) -> Result<Value, Failure> {
    let (owner, name) = target
        .path
        .rsplit_once('/')
        .ok_or_else(|| Failure::Body("the remote names no owner".into()))?;
    let mut variables = json!({ "owner": owner, "name": name });
    for (index, branch) in target.branches.iter().enumerate() {
        variables[format!("b{index}")] = json!(branch);
    }
    let answer = http::post(
        agent,
        "https://api.github.com/graphql",
        token,
        &json!({ "query": query(target.branches.len()), "variables": variables }),
    )?;
    if answer.get("data").is_none_or(Value::is_null)
        && let Some(errors) = answer.get("errors")
    {
        let words = errors.get(0).map(http::message).unwrap_or_default();
        return Err(Failure::Status(200, words));
    }
    let data = &answer["data"];
    let repository = &data["repository"];
    if repository.is_null() {
        return Err(Failure::Status(
            404,
            format!("{} is not visible to this account", target.path),
        ));
    }
    // Pictures first, so this very publish shows them.
    avatars.fetch(agent, &people(data));
    Ok(shape(data, target, avatars))
}

/// Every (login, avatar address) the answer names.
fn people(data: &Value) -> Vec<(String, String)> {
    let mut found = Vec::new();
    let mut add = |person: &Value| {
        if let (Some(login), Some(url)) = (
            person.get("login").and_then(Value::as_str),
            person.get("avatarUrl").and_then(Value::as_str),
        ) && !found
            .iter()
            .any(|(seen, _): &(String, String)| seen == login)
        {
            found.push((login.to_owned(), url.to_owned()));
        }
    };
    add(&data["viewer"]);
    for pr in nodes(&data["repository"]["pullRequests"]) {
        add(&pr["author"]);
    }
    found
}

fn nodes(connection: &Value) -> impl Iterator<Item = &Value> {
    connection["nodes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|node| node.is_object())
}

pub(crate) fn shape(data: &Value, target: &Target, avatars: &Avatars) -> Value {
    let repository = &data["repository"];
    let viewer = &data["viewer"];
    let prs: Vec<Value> = nodes(&repository["pullRequests"])
        .map(|pr| pull_request(pr, avatars, false))
        .collect();
    let issues: Vec<Value> = nodes(&repository["issues"])
        .map(|issue| {
            json!({
                "number": issue["number"],
                "title": shape::text(issue.get("title")),
                "url": shape::url(issue.get("url")),
                "author": author(&issue["author"], avatars),
                "updated": shape::seconds(issue.get("updatedAt")),
                "labels": nodes(&issue["labels"]).map(|label| json!({
                    "name": shape::text(label.get("name")),
                    "color": shape::text(label.get("color")),
                })).collect::<Vec<_>>(),
                "assignees": nodes(&issue["assignees"]).map(|person| person["login"].clone()).collect::<Vec<_>>(),
            })
        })
        .collect();
    let mut branches = Map::new();
    let mut current = Value::Null;
    for (index, branch) in target.branches.iter().enumerate() {
        let Some(pr) = nodes(&repository[format!("b{index}")]).next() else {
            continue;
        };
        let is_current = *branch == target.branch;
        let pr = pull_request(pr, avatars, is_current);
        if is_current {
            current = pr.clone();
        }
        branches.insert(branch.clone(), pr);
    }
    json!({
        "repo": {
            "name_with_owner": shape::text(repository.get("nameWithOwner")),
            "url": shape::url(repository.get("url")),
            "default_branch": shape::text(repository["defaultBranchRef"].get("name")),
        },
        "viewer": shape::person(&viewer["login"], &shape::text(viewer.get("name")), false, &viewer["avatarUrl"], avatars),
        "prs": prs,
        "current_pr": current,
        "branches": branches,
        "issues": issues,
    })
}

fn author(author: &Value, avatars: &Avatars) -> Value {
    shape::person(
        &author["login"],
        &shape::text(author.get("name")),
        author["__typename"] == "Bot",
        &author["avatarUrl"],
        avatars,
    )
}

/// What a check's outcome counts as.
fn outcome(check: &Value) -> &'static str {
    let conclusion = check
        .get("conclusion")
        .or_else(|| check.get("state"))
        .and_then(Value::as_str);
    let running = check
        .get("status")
        .and_then(Value::as_str)
        .is_some_and(|status| status != "COMPLETED");
    match conclusion {
        _ if running && check.get("conclusion").is_none_or(Value::is_null) => "pending",
        Some("SUCCESS") => "passed",
        Some("NEUTRAL" | "SKIPPED") => "skipped",
        None | Some("PENDING" | "EXPECTED" | "QUEUED" | "IN_PROGRESS") => "pending",
        Some(_) => "failed",
    }
}

fn pull_request(pr: &Value, avatars: &Avatars, details: bool) -> Value {
    let lower = |key: &str| {
        pr.get(key)
            .and_then(Value::as_str)
            .map_or(String::new(), str::to_ascii_lowercase)
    };
    let state = lower("state");
    let review = lower("reviewDecision");
    let mergeable = lower("mergeable");
    let merge_state = lower("mergeStateStatus");
    let contexts: Vec<&Value> =
        nodes(&pr["commits"]["nodes"][0]["commit"]["statusCheckRollup"]["contexts"])
            .take(CHECKS)
            .collect();
    let mut checks = Checks::default();
    for check in &contexts {
        checks.add(outcome(check));
    }
    let standing = Standing {
        state: &state,
        draft: pr["isDraft"] == true,
        conflicts: merge_state == "dirty"
            || mergeable == "conflicting"
            || (merge_state == "unstable" && checks.pending == 0),
        changes_requested: review == "changes_requested",
        review_required: review == "review_required",
        ready: merge_state == "clean",
        held: merge_state == "blocked" || merge_state == "behind",
        checks,
    };
    let mut value = json!({
        "number": pr["number"],
        "title": shape::text(pr.get("title")),
        "state": state,
        "draft": standing.draft,
        "url": shape::url(pr.get("url")),
        "branch": shape::text(pr.get("headRefName")),
        "base": shape::text(pr.get("baseRefName")),
        "author": author(&pr["author"], avatars),
        "updated": shape::seconds(pr.get("updatedAt")),
        "review_decision": review,
        "mergeable": mergeable,
        "merge_state": merge_state,
        "additions": pr["additions"],
        "deletions": pr["deletions"],
        "files": pr["changedFiles"],
        "checks": checks.json(),
        "tone": standing.tone(),
    });
    if details {
        value["check_list"] = contexts
            .iter()
            .map(|check| {
                json!({
                    "name": shape::text(check.get("name").or_else(|| check.get("context"))),
                    "outcome": outcome(check),
                    "url": shape::url(check.get("detailsUrl").or_else(|| check.get("targetUrl"))),
                })
            })
            .collect();
    }
    value
}

/// Who `token` belongs to, to check a token before keeping it.
pub fn login(agent: &ureq::Agent, token: &SecretString) -> Result<String, Failure> {
    let user = http::get(agent, "https://api.github.com/user", token)?;
    user.get("login")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| Failure::Body("no login".into()))
}
