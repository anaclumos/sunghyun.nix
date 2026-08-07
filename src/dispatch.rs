use crate::actions::{clipboard, input_source, launcher, open, tile};
use crate::config::Config;
use crate::error::{ActionError, ActionResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Open(String),
    OpenDefaultBrowser,
    InputSource(String),
    Tile(String),
    Launcher { query: Option<String> },
    ClipboardShow,
    ClipboardCapture,
    ClipboardPaste(usize),
}

pub fn dispatch(config: &Config, action: &Action) -> ActionResult {
    match action {
        Action::Open(target) => open::open_target(config, target),
        Action::OpenDefaultBrowser => open::open_default_browser(),
        Action::InputSource(name) => input_source::switch(config, name),
        Action::Tile(name) => tile::tile(config, name),
        Action::Launcher { query } => launcher::launch(config, query.as_deref()),
        Action::ClipboardShow => clipboard::show(config),
        Action::ClipboardCapture => clipboard::capture(config),
        Action::ClipboardPaste(i) => clipboard::paste_index(config, *i),
    }
}

pub fn parse_action(kind: &str, arg: Option<&str>) -> Result<Action, ActionError> {
    match kind {
        "open-default-browser" | "open-browser" => Ok(Action::OpenDefaultBrowser),
        "open" => {
            let target = arg.ok_or_else(|| ActionError::failed("open requires target"))?;
            match target.to_ascii_lowercase().as_str() {
                "browser" | "default-browser" | "default_browser" => {
                    Ok(Action::OpenDefaultBrowser)
                }
                _ => Ok(Action::Open(target.into())),
            }
        }
        "input-source" => Ok(Action::InputSource(
            arg.ok_or_else(|| ActionError::failed("input-source requires name"))?
                .into(),
        )),
        "tile" => Ok(Action::Tile(
            arg.ok_or_else(|| ActionError::failed("tile requires action"))?
                .into(),
        )),
        "launcher" => Ok(Action::Launcher {
            query: arg.map(|s| s.to_string()),
        }),
        "clipboard" | "clipboard-show" => Ok(Action::ClipboardShow),
        "clipboard-capture" => Ok(Action::ClipboardCapture),
        "clipboard-paste" => {
            let i: usize = arg
                .ok_or_else(|| ActionError::failed("clipboard-paste requires index"))?
                .parse()
                .map_err(|_| ActionError::failed("clipboard-paste index must be usize"))?;
            Ok(Action::ClipboardPaste(i))
        }
        other => Err(ActionError::failed(format!("unknown action: {other}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::headless;

    #[test]
    fn parse_and_dispatch_tile_headless_skip() {
        headless::force(true);
        let cfg = Config::default();
        let action = parse_action("tile", Some("left")).unwrap();
        let err = dispatch(&cfg, &action).unwrap_err();
        assert!(matches!(err, ActionError::Skipped(_)));
    }

    #[test]
    fn parse_open() {
        let a = parse_action("open", Some("ghostty")).unwrap();
        assert_eq!(a, Action::Open("ghostty".into()));
    }

    #[test]
    fn parse_open_browser_aliases() {
        assert_eq!(
            parse_action("open", Some("browser")).unwrap(),
            Action::OpenDefaultBrowser
        );
        assert_eq!(
            parse_action("open-default-browser", None).unwrap(),
            Action::OpenDefaultBrowser
        );
    }
}
