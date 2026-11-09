//! X11 window identity only: no titles or clipboard contents are inspected.
use crate::protocol::ErrorCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FocusTarget {
    pub active_window: u64,
    pub focused_window: u64,
}

fn window_id(raw: &[u8]) -> Result<u64, ErrorCode> {
    let value = std::str::from_utf8(raw)
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok())
        .filter(|value| *value > 1)
        .ok_or(ErrorCode::FocusChanged)?;
    Ok(u64::from(value))
}

pub(crate) fn capture(
    mut run: impl FnMut(&[&str]) -> Result<Vec<u8>, ErrorCode>,
) -> Result<FocusTarget, ErrorCode> {
    let active = window_id(&run(&["getactivewindow"])?)?;
    // -f keeps the real focus child instead of finding a WM_CLASS ancestor.
    let focused = window_id(&run(&["getwindowfocus", "-f"])?)?;
    for window in [active, focused] {
        run(&["getwindowgeometry", &window.to_string()])?;
    }
    let owner = run(&["getwindowpid", &active.to_string()])?;
    let owner = std::str::from_utf8(&owner)
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok())
        .filter(|value| *value > 0)
        .ok_or(ErrorCode::FocusChanged)?;
    if owner == std::process::id()
        || window_id(&run(&["getactivewindow"])?)? != active
        || window_id(&run(&["getwindowfocus", "-f"])?)? != focused
    {
        return Err(ErrorCode::FocusChanged);
    }
    Ok(FocusTarget {
        active_window: active,
        focused_window: focused,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_ids_are_strict_positive_x11_identifiers() {
        assert_eq!(window_id(b"123\n"), Ok(123));
        for value in [
            b"0".as_slice(),
            b"1",
            b"-1",
            b"hello",
            b"2 3",
            b"4294967296",
        ] {
            assert!(window_id(value).is_err());
        }
    }
}
