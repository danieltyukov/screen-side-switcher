use screen_side_core::model::State;
use screen_side_core::Error;

/// A screen by its number in `status`, its connector, or part of its name.
pub fn resolve_screen(state: &State, selector: &str) -> Result<String, Error> {
    let listed = state.listed();
    let wanted = selector.trim();
    let chosen = if let Ok(n) = wanted.parse::<usize>() {
        listed.get(n.wrapping_sub(1)).copied().ok_or_else(|| {
            Error::Usage(format!(
                "There is no screen {n}. Run screen-side status to see the numbers."
            ))
        })?
    } else if let Some(s) = listed
        .iter()
        .find(|s| s.connector.eq_ignore_ascii_case(wanted))
    {
        s
    } else {
        let lower = wanted.to_lowercase();
        let hits: Vec<_> = listed
            .iter()
            .enumerate()
            .filter(|(_, s)| s.name.to_lowercase().contains(&lower))
            .map(|(i, s)| (i, *s))
            .collect();
        match hits.as_slice() {
            [(_, s)] => *s,
            [] => {
                return Err(Error::Usage(format!(
                    "No screen matches '{wanted}'. Run screen-side status to see them."
                )))
            }
            many => {
                let names: Vec<String> = many
                    .iter()
                    .map(|(i, s)| format!("{} {} {}", i + 1, s.connector, s.name))
                    .collect();
                return Err(Error::Usage(format!(
                    "'{wanted}' matches {} screens: {}. Use the number or the connector.",
                    many.len(),
                    names.join(", ")
                )));
            }
        }
    };
    if !chosen.enabled {
        return Err(Error::Usage(format!(
            "{} ({}) is switched off.",
            chosen.name, chosen.connector
        )));
    }
    Ok(chosen.id.clone())
}
