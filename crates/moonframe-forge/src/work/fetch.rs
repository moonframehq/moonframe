//! Fetch/renew ordering, keeping transient renewal errors distinct from
//! a refused token and returning every adopted credential to the provider.

use {
    crate::{
        credentials::{Credential, Renewal, Source},
        http::Failure,
    },
    serde_json::Value,
};

pub(super) fn read(
    mut credential: Credential,
    mut renew: impl FnMut(&Credential) -> Renewal,
    mut read: impl FnMut(&Credential) -> Result<Value, Failure>,
) -> (Result<Value, Failure>, Option<Credential>) {
    let mut updated = credential.source == Source::Cli;
    let result = (|| {
        let renewed = if credential.expiring() {
            adopt(renew(&credential), &mut credential, &mut updated)?
        } else {
            false
        };
        let mut result = read(&credential);
        if result == Err(Failure::Unauthorized) && !renewed && credential.refresh.is_some() {
            adopt(renew(&credential), &mut credential, &mut updated)?;
            result = read(&credential);
        }
        result
    })();
    (result, updated.then_some(credential))
}

fn adopt(
    renewal: Renewal,
    credential: &mut Credential,
    updated: &mut bool,
) -> Result<bool, Failure> {
    let (fresh, renewed) = match renewal {
        Renewal::Failed(failure) => return Err(failure),
        Renewal::Gone => return Err(Failure::Unauthorized),
        Renewal::Newer(fresh) => (fresh, false),
        Renewal::Renewed(fresh) => (fresh, true),
    };
    *credential = fresh;
    *updated = true;
    Ok(renewed)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
