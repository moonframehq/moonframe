//! `app:dock`: what the app's Dock icon says while its window is out of
//! sight: a short badge, a progress bar, or neither. Only macOS has one;
//! elsewhere it does nothing.

use {
    super::APP,
    crate::{Data, Error},
};

/// Longest a badge can be: the Dock shows a few characters.
const MAX_BADGE: usize = 16;

/// What `app:dock` changes; what it leaves out stays as it is.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Dock {
    /// The badge to show, or `Some(None)` to take it off.
    pub badge: Option<Option<String>>,
    /// The fraction to show, or `Some(None)` to take the bar off.
    pub progress: Option<Option<f64>>,
}

/// `{ badge = "3", progress = 0.4 }`, either or both, each `false` to take
/// it off.
pub(super) fn dock(msg: &Data) -> Result<Dock, Error> {
    let fail = |problem: &str| Error::new(format!("`{APP}dock`: {problem}"));
    let usage = "takes { badge = \"3\", progress = 0.4 }, either or both, or false to clear one";
    let Data::Map(fields) = msg else {
        return Err(fail(usage));
    };
    if fields.is_empty() {
        return Err(fail(usage));
    }
    if let Some(key) = fields
        .keys()
        .find(|key| !matches!(key.as_str(), "badge" | "progress"))
    {
        return Err(fail(&format!(
            "unknown key `{key}`; it takes badge and progress"
        )));
    }
    let badge = match fields.get("badge") {
        None => None,
        Some(Data::Bool(false)) => Some(None),
        Some(Data::String(text)) if !text.is_empty() && text.len() <= MAX_BADGE => {
            Some(Some(text.clone()))
        },
        Some(Data::Integer(count)) => Some(Some(count.to_string())),
        Some(_) => {
            return Err(fail(&format!(
                "`badge` is a count, text of at most {MAX_BADGE} bytes, or false"
            )));
        },
    };
    let fraction = |value: f64| {
        (0.0..=1.0)
            .contains(&value)
            .then_some(Some(Some(value)))
            .ok_or_else(|| fail("`progress` is 0 to 1, or false"))
    };
    let progress = match fields.get("progress") {
        None => None,
        Some(Data::Bool(false)) => Some(None),
        Some(Data::Number(value)) => fraction(*value)?,
        Some(Data::Integer(value)) => fraction(*value as f64)?,
        Some(_) => return Err(fail("`progress` is 0 to 1, or false")),
    };
    Ok(Dock { badge, progress })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use {super::*, std::collections::BTreeMap};

    fn map(fields: &[(&str, Data)]) -> Data {
        Data::Map(
            fields
                .iter()
                .map(|(key, value)| ((*key).to_owned(), value.clone()))
                .collect::<BTreeMap<_, _>>(),
        )
    }

    #[test]
    fn it_reads_what_it_changes_and_leaves_the_rest() {
        assert_eq!(
            dock(&map(&[
                ("badge", Data::Integer(3)),
                ("progress", Data::Number(0.4))
            ]))
            .unwrap(),
            Dock {
                badge: Some(Some("3".to_owned())),
                progress: Some(Some(0.4)),
            }
        );
        assert_eq!(
            dock(&map(&[("progress", Data::Bool(false))])).unwrap(),
            Dock {
                badge: None,
                progress: Some(None),
            }
        );
    }

    #[test]
    fn it_refuses_what_it_cannot_read() {
        for msg in [
            Data::from("3"),
            map(&[]),
            map(&[("nope", Data::Bool(true))]),
            map(&[("badge", Data::Bool(true))]),
            map(&[("badge", Data::from(""))]),
            map(&[("progress", Data::Number(1.5))]),
        ] {
            assert!(dock(&msg).is_err(), "{msg:?}");
        }
    }
}
