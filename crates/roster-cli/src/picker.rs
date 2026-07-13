use anyhow::Result;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    style::{Attribute, Print, SetAttribute},
    terminal::{self, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use roster_core::{Agent, Roster};
use std::io::{self, Write};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

struct PickItem {
    name: String,
    agent: Agent,
    error: Option<String>,
}

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> Result<Self> {
        terminal::enable_raw_mode()?;
        let guard = Self;
        let mut output = io::stdout();
        execute!(output, EnterAlternateScreen, cursor::Hide)?;
        Ok(guard)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let mut output = io::stdout();
        let _ = execute!(
            output,
            SetAttribute(Attribute::Reset),
            cursor::Show,
            LeaveAlternateScreen
        );
        let _ = terminal::disable_raw_mode();
    }
}

pub fn pick(roster: &Roster) -> Result<Option<String>> {
    let items = roster
        .agents()
        .iter()
        .map(|(name, agent)| PickItem {
            name: name.clone(),
            agent: agent.clone(),
            error: roster.resolve(name).err().map(|error| error.to_string()),
        })
        .collect::<Vec<_>>();
    let _guard = TerminalGuard::enter()?;
    let mut output = io::stdout();
    pick_loop(
        &items,
        &mut output,
        || Ok(event::read()?),
        || Ok(terminal::size()?),
    )
}

fn pick_loop(
    items: &[PickItem],
    output: &mut impl Write,
    mut read_event: impl FnMut() -> Result<Event>,
    mut terminal_size: impl FnMut() -> Result<(u16, u16)>,
) -> Result<Option<String>> {
    let mut query = String::new();
    let mut selected = 0usize;
    loop {
        let needle = query.to_lowercase();
        let matches = items
            .iter()
            .filter(|item| {
                item.name.to_lowercase().contains(&needle)
                    || item.agent.description.to_lowercase().contains(&needle)
                    || item.agent.model.to_lowercase().contains(&needle)
            })
            .collect::<Vec<_>>();
        selected = selected.min(matches.len().saturating_sub(1));
        render(output, &query, &matches, selected, terminal_size()?)?;

        if let Event::Key(key) = read_event()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Esc => return Ok(None),
                KeyCode::Enter => {
                    if let Some(item) = matches.get(selected)
                        && item.error.is_none()
                    {
                        return Ok(Some(item.name.clone()));
                    }
                }
                KeyCode::Up => selected = selected.saturating_sub(1),
                KeyCode::Down => selected = (selected + 1).min(matches.len().saturating_sub(1)),
                KeyCode::Backspace => {
                    query.pop();
                    selected = 0;
                }
                KeyCode::Char(character) => {
                    query.push(character);
                    selected = 0;
                }
                _ => {}
            }
        }
    }
}

fn render(
    output: &mut impl Write,
    query: &str,
    matches: &[&PickItem],
    selected: usize,
    size: (u16, u16),
) -> Result<()> {
    let (terminal_width, terminal_height) = size;
    let width = usize::from(terminal_width);
    let height = usize::from(terminal_height);
    execute!(
        output,
        cursor::MoveTo(0, 0),
        terminal::Clear(ClearType::All)
    )?;
    if width == 0 || height == 0 {
        return Ok(());
    }

    let list_top = match height {
        0..=2 => 0,
        3 => 1,
        4..=5 => 2,
        _ => 4,
    };
    if height >= 3 {
        draw_line(output, 0, width, "Roster / Launch agent", false)?;
    }
    if height >= 4 {
        draw_line(output, 1, width, &format!("Search  > {query}"), false)?;
    }
    if height >= 6 {
        let heading = if width >= 56 {
            "  AGENT              HARNESS  PURPOSE"
        } else {
            "  AGENT              HARNESS"
        };
        draw_line(output, 3, width, heading, false)?;
    }

    let detail_height = usize::from(height >= 12) * 5;
    let footer_height = usize::from(height >= 2);
    let visible_rows = height
        .saturating_sub(list_top + detail_height + footer_height)
        .min(12);
    let start = list_window_start(selected, matches.len(), visible_rows);
    if matches.is_empty() {
        if list_top < height {
            draw_line(output, list_top, width, "  No matching agents", false)?;
        }
    } else {
        for (row_offset, (index, item)) in matches
            .iter()
            .enumerate()
            .skip(start)
            .take(visible_rows)
            .enumerate()
        {
            let harness = if item.error.is_some() {
                "disabled".to_owned()
            } else {
                item.agent.harness.to_string()
            };
            let name = clip_to_width(&item.name, 18);
            let harness = clip_to_width(&harness, 8);
            let marker = if index == selected { '›' } else { ' ' };
            let line = if width >= 56 {
                format!(
                    "{marker} {name:<18} {harness:<8} {}",
                    item.agent.description
                )
            } else {
                format!("{marker} {name:<18} {harness:<8}")
            };
            let row = list_top + row_offset;
            if row < height {
                draw_line(output, row, width, &line, index == selected)?;
            }
        }
    }

    if detail_height > 0
        && let Some(item) = matches.get(selected)
    {
        let top = height - detail_height - 1;
        draw_line(
            output,
            top,
            width,
            &format!("{} · {}", item.name.to_uppercase(), item.agent.description),
            false,
        )?;
        draw_line(
            output,
            top + 1,
            width,
            &format!("Role     {}", item.agent.role),
            false,
        )?;
        draw_line(
            output,
            top + 2,
            width,
            &format!(
                "Harness  {}    Model  {}",
                item.agent.harness, item.agent.model
            ),
            false,
        )?;
        let status = item
            .error
            .as_deref()
            .map(|error| format!("Disabled · {error}"))
            .unwrap_or_else(|| "Ready".to_owned());
        draw_line(
            output,
            top + 3,
            width,
            &format!(
                "Status   {status}                                      {} / {}",
                selected + 1,
                matches.len()
            ),
            false,
        )?;
    }

    let footer = if matches
        .get(selected)
        .is_some_and(|item| item.error.is_none())
    {
        "↑↓ move   type to filter   Enter launch   Esc cancel"
    } else {
        "↑↓ move   type to filter   Esc cancel"
    };
    if footer_height > 0 {
        draw_line(output, height - 1, width, footer, false)?;
    }
    output.flush()?;
    Ok(())
}

fn draw_line(
    output: &mut impl Write,
    row: usize,
    width: usize,
    text: &str,
    selected: bool,
) -> Result<()> {
    let clipped = clip_to_width(text, width);
    execute!(
        output,
        cursor::MoveTo(0, row as u16),
        terminal::Clear(ClearType::CurrentLine)
    )?;
    if selected {
        execute!(output, SetAttribute(Attribute::Reverse))?;
        let padding = width.saturating_sub(UnicodeWidthStr::width(clipped.as_str()));
        write!(output, "{clipped}{}", " ".repeat(padding))?;
        execute!(output, SetAttribute(Attribute::Reset))?;
    } else {
        execute!(output, Print(clipped))?;
    }
    Ok(())
}

fn clip_to_width(text: &str, width: usize) -> String {
    if UnicodeWidthStr::width(text) <= width {
        return text.to_owned();
    }
    if width == 0 {
        return String::new();
    }
    if width == 1 {
        return "…".to_owned();
    }
    let target = width - 1;
    let mut used = 0usize;
    let mut clipped = String::new();
    for character in text.chars() {
        let character_width = UnicodeWidthChar::width(character).unwrap_or(0);
        if used + character_width > target {
            break;
        }
        clipped.push(character);
        used += character_width;
    }
    clipped.push('…');
    clipped
}

fn list_window_start(selected: usize, total: usize, visible: usize) -> usize {
    if visible == 0 || total <= visible {
        return 0;
    }
    (selected + 1).saturating_sub(visible).min(total - visible)
}

#[cfg(test)]
mod tests {
    use super::{PickItem, clip_to_width, list_window_start, pick_loop, render};
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
    use roster_core::{Agent, Harness};
    use std::collections::VecDeque;
    use unicode_width::UnicodeWidthStr;

    fn item(name: &str, description: &str, error: Option<&str>) -> PickItem {
        PickItem {
            name: name.to_owned(),
            agent: Agent {
                description: description.to_owned(),
                role: "core/role:test".to_owned(),
                model: "gpt-test".to_owned(),
                reasoning: Some("high".to_owned()),
                harness: Harness::Codex,
                args: Vec::new(),
                delegates: Vec::new(),
            },
            error: error.map(str::to_owned),
        }
    }

    fn press(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    #[test]
    fn clipping_prevents_terminal_soft_wraps() {
        assert_eq!(clip_to_width("abcdef", 6), "abcdef");
        assert_eq!(clip_to_width("abcdef", 5), "abcd…");
        assert_eq!(clip_to_width("abcdef", 1), "…");
        assert_eq!(clip_to_width("abcdef", 0), "");
        assert_eq!(clip_to_width("界界界", 5), "界界…");
        assert_eq!(
            UnicodeWidthStr::width(clip_to_width("界界界", 4).as_str()),
            3
        );
    }

    #[test]
    fn list_window_keeps_the_selection_visible() {
        assert_eq!(list_window_start(0, 20, 5), 0);
        assert_eq!(list_window_start(4, 20, 5), 0);
        assert_eq!(list_window_start(5, 20, 5), 1);
        assert_eq!(list_window_start(19, 20, 5), 15);
        assert_eq!(list_window_start(0, 0, 5), 0);
    }

    #[test]
    fn renderer_keeps_full_narrow_and_compact_layouts_legible() {
        let enabled = item("amos", "Codex orchestrator", None);
        let disabled = item("broken", "Unavailable agent", Some("missing role"));
        let items = [&enabled, &disabled];

        let mut full = Vec::new();
        render(&mut full, "", &items, 0, (80, 12)).unwrap();
        let full = String::from_utf8(full).unwrap();
        assert!(full.contains("AGENT"));
        assert!(full.contains("AMOS · Codex orchestrator"));
        assert!(full.contains("Enter launch"));

        let mut narrow = Vec::new();
        render(&mut narrow, "界界界界界", &[], 0, (20, 10)).unwrap();
        let narrow = String::from_utf8(narrow).unwrap();
        assert!(narrow.contains("No matching agents"));
        assert!(narrow.contains('…'));

        for height in 1..=5 {
            let mut compact = Vec::new();
            render(&mut compact, "", &[&enabled], 0, (24, height)).unwrap();
            assert!(String::from_utf8(compact).unwrap().contains("amos"));
        }

        let mut zero = Vec::new();
        render(&mut zero, "", &[&enabled], 0, (0, 0)).unwrap();

        let mut unavailable = Vec::new();
        render(&mut unavailable, "", &[&disabled], 0, (80, 12)).unwrap();
        let unavailable = String::from_utf8(unavailable).unwrap();
        assert!(unavailable.contains("Disabled · missing role"));
        assert!(!unavailable.contains("Enter launch"));
    }

    #[test]
    fn picker_filters_navigates_edits_and_launches() {
        let items = [
            item("amos", "Orchestrator", None),
            item("cerberus", "Reviewer", None),
            item("eames", "Designer", None),
        ];
        let mut release = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
        release.kind = KeyEventKind::Release;
        let mut events = VecDeque::from([
            Event::Resize(80, 12),
            Event::Key(release),
            press(KeyCode::Char('e')),
            press(KeyCode::Down),
            press(KeyCode::Up),
            press(KeyCode::Backspace),
            press(KeyCode::Down),
            press(KeyCode::Left),
            press(KeyCode::Enter),
        ]);
        let mut output = Vec::new();
        let picked = pick_loop(
            &items,
            &mut output,
            || Ok(events.pop_front().expect("test event")),
            || Ok((80, 12)),
        )
        .unwrap();
        assert_eq!(picked.as_deref(), Some("cerberus"));
        assert!(String::from_utf8(output).unwrap().contains("Search  > e"));
    }

    #[test]
    fn disabled_agent_cannot_launch_and_escape_cancels() {
        let items = [item("broken", "Unavailable", Some("missing role"))];
        let mut events = VecDeque::from([press(KeyCode::Enter), press(KeyCode::Esc)]);
        let mut output = Vec::new();
        let picked = pick_loop(
            &items,
            &mut output,
            || Ok(events.pop_front().expect("test event")),
            || Ok((80, 12)),
        )
        .unwrap();
        assert_eq!(picked, None);
    }
}
