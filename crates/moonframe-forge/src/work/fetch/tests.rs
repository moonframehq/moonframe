use {
    super::*,
    secrecy::{ExposeSecret, SecretString},
    serde_json::json,
};

fn oauth(expiring: bool) -> Credential {
    Credential {
        token: SecretString::from("access-token"),
        refresh: Some(SecretString::from("refresh-token")),
        expires: expiring.then_some(1),
        source: Source::Saved,
    }
}

#[test]
fn transient_renewal_errors_do_not_become_sign_outs() {
    for failure in [
        Failure::Network("offline".into()),
        Failure::RateLimited,
        Failure::Status(503, "unavailable".into()),
        Failure::Body("incomplete answer".into()),
    ] {
        let (result, updated) = read(
            oauth(true),
            |_| Renewal::Failed(failure.clone()),
            |_| panic!("must not use the expired access token after a failed renewal"),
        );
        assert_eq!(result, Err(failure.clone()));
        assert!(updated.is_none());

        // The API may discover expiry before the local clock does. A
        // renewal outage must replace that 401 with a retryable failure.
        let mut reads = 0;
        let (result, updated) = read(
            oauth(false),
            |_| Renewal::Failed(failure.clone()),
            |_| {
                reads += 1;
                Err(Failure::Unauthorized)
            },
        );
        assert_eq!(reads, 1);
        assert_eq!(result, Err(failure));
        assert!(updated.is_none());
    }
}

#[test]
fn a_refused_or_forgotten_refresh_still_signs_out() {
    for gone in [false, true] {
        let (result, _) = read(
            oauth(true),
            |_| {
                if gone {
                    Renewal::Gone
                } else {
                    Renewal::Failed(Failure::Unauthorized)
                }
            },
            |_| panic!("no valid sign-in"),
        );
        assert_eq!(result, Err(Failure::Unauthorized));
    }
}

#[test]
fn an_adopted_pair_can_still_renew_and_a_renewed_pair_is_returned_on_failure() {
    let mut renewals = 0;
    let mut reads = 0;
    let (result, updated) = read(
        oauth(true),
        |_| {
            renewals += 1;
            let mut fresh = oauth(false);
            if renewals == 1 {
                fresh.token = SecretString::from("adopted-token");
                Renewal::Newer(fresh)
            } else {
                fresh.token = SecretString::from("renewed-token");
                Renewal::Renewed(fresh)
            }
        },
        |credential| {
            reads += 1;
            if reads == 1 {
                assert_eq!(credential.token.expose_secret(), "adopted-token");
                Err(Failure::Unauthorized)
            } else {
                assert_eq!(credential.token.expose_secret(), "renewed-token");
                Err(Failure::RateLimited)
            }
        },
    );
    assert_eq!(renewals, 2);
    assert_eq!(result, Err(Failure::RateLimited));
    assert_eq!(updated.unwrap().token.expose_secret(), "renewed-token");
}

#[test]
fn a_successful_renewal_retries_the_fetch_once() {
    let mut reads = 0;
    let (result, updated) = read(
        oauth(false),
        |_| Renewal::Renewed(oauth(false)),
        |_| {
            reads += 1;
            if reads == 1 {
                Err(Failure::Unauthorized)
            } else {
                Ok(json!({ "ok": true }))
            }
        },
    );
    assert_eq!(reads, 2);
    assert!(result.is_ok());
    assert!(updated.is_some());
}
