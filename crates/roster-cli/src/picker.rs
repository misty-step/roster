use anyhow::Result;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    style::{Attribute, Print, SetAttribute},
    terminal::{self, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use roster_core::Roster;
use std::io::{self, Write};

struct PickItem<'a> {
    name: &'a String,
    agent: &'a roster_core::Agent,
    error: Option<String>,
}

pub fn pick(roster: &Roster) -> Result<Option<String>> {
    terminal::enable_raw_mode()?;
    let mut output = io::stdout();
    execute!(output, EnterAlternateScreen, cursor::Hide)?;
    let result = pick_loop(roster, &mut output);
    execute!(output, cursor::Show, LeaveAlternateScreen)?;
    terminal::disable_raw_mode()?;
    result
}

fn pick_loop(roster: &Roster, output: &mut impl Write) -> Result<Option<String>> {
    let mut query = String::new();
    let mut selected = 0usize;
    loop {
        let matches = roster
            .agents()
            .iter()
            .filter(|(name, agent)| {
                let needle = query.to_lowercase();
                name.to_lowercase().contains(&needle)
                    || agent.description.to_lowercase().contains(&needle)
                    || agent.model.to_lowercase().contains(&needle)
            })
            .map(|(name, agent)| PickItem {
                name,
                agent,
                error: roster.resolve(name).err().map(|error| error.to_string()),
            })
            .collect::<Vec<_>>();
        selected = selected.min(matches.len().saturating_sub(1));
        execute!(
            output,
            cursor::MoveTo(0, 0),
            terminal::Clear(ClearType::All),
            Print("ROSTER  Pick an agent\n\n"),
            Print(format!("> {query}\n\n"))
        )?;
        for (index, item) in matches.iter().take(12).enumerate() {
            if index == selected {
                execute!(output, SetAttribute(Attribute::Reverse))?;
            }
            execute!(
                output,
                Print(format!(
                    "  {:<18} {:<8} {}\n",
                    item.name,
                    if item.error.is_some() {
                        "disabled".to_owned()
                    } else {
                        item.agent.harness.to_string()
                    },
                    item.agent.description
                )),
                SetAttribute(Attribute::Reset)
            )?;
        }
        if let Some(item) = matches.get(selected) {
            execute!(
                output,
                Print("\nEFFECTIVE\n"),
                Print(format!("  agent    {}\n", item.name)),
                Print(format!("  role     {}\n", item.agent.role)),
                Print(format!("  harness  {}\n", item.agent.harness)),
                Print(format!("  model    {}\n", item.agent.model))
            )?;
            if let Some(error) = &item.error {
                execute!(
                    output,
                    Print(format!("  status   DISABLED: {error}\n")),
                    Print("\nEsc cancel  type to filter")
                )?;
            } else {
                execute!(output, Print("\nEnter launch  Esc cancel  type to filter"))?;
            }
        }
        output.flush()?;

        if let Event::Key(key) = event::read()? {
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
