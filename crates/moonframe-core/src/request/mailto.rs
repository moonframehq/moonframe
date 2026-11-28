//! `app:open` of a `mailto:` address: one person's address, opened by the
//! system's mail app. Only the address: no headers (`?subject=`, `?body=`,
//! `?cc=`) and no list, so a mod cannot have a mail written on its behalf.

/// Longest address after `mailto:`, as an address's own limit.
const MAX_ADDRESS: usize = 254;

/// Whether `url` is `mailto:` and one address: a local part and a domain,
/// with no space, control, or character that starts a header or a list.
pub(super) fn is_mailto(url: &str) -> bool {
    let Some(address) = url
        .get(..7)
        .filter(|scheme| scheme.eq_ignore_ascii_case("mailto:"))
        .and_then(|_| url.get(7..))
    else {
        return false;
    };
    let Some((local, domain)) = address.split_once('@') else {
        return false;
    };
    let plain = |part: &str| {
        !part.is_empty()
            && part.chars().all(|c| {
                !c.is_whitespace() && !c.is_control() && !"@?#&,;<>\"()[]\\/:%".contains(c)
            })
    };
    address.len() <= MAX_ADDRESS
        && plain(local)
        && plain(domain)
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
}
