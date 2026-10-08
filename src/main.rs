mod api;
mod icon;

use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::time::Duration;

use chrono::{DateTime, Datelike, Local, Utc};
use ksni::blocking::TrayMethods;
use ksni::menu::{MenuItem, StandardItem};
use ksni::{Icon, ToolTip, Tray};

use api::{Limit, Usage};

const DEFAULT_INTERVAL_SECS: u64 = 300;
const USAGE_PAGE: &str = "https://claude.ai/settings/usage";

enum State {
    Loading,
    Ok(Usage, DateTime<Local>),
    Error(String, Option<Usage>),
}

struct UsageTray {
    state: State,
    refresh: Sender<()>,
}

impl UsageTray {
    fn usage(&self) -> Option<&Usage> {
        match &self.state {
            State::Ok(u, _) => Some(u),
            State::Error(_, u) => u.as_ref(),
            State::Loading => None,
        }
    }

    /// Líneas de resumen, con el porcentaje cuando la línea es un límite.
    fn summary_lines(&self) -> Vec<(String, Option<f64>)> {
        let mut lines = Vec::new();
        if let Some(u) = self.usage() {
            lines.extend(
                [&u.session, &u.weekly]
                    .into_iter()
                    .flatten()
                    .chain(&u.others)
                    .map(|l| (format_limit(l), Some(l.percent))),
            );
        }
        match &self.state {
            State::Loading => lines.push(("Consultando…".into(), None)),
            State::Error(e, _) => lines.push((format!("⚠ {e}"), None)),
            State::Ok(..) => {}
        }
        lines
    }
}

fn format_limit(l: &Limit) -> String {
    let mut s = format!("{}: {:.0}%", l.label, l.percent);
    if let Some(r) = l.resets_at {
        s.push_str(&format!(" · reinicia {}", format_reset(r)));
    }
    s
}

fn format_reset(at: DateTime<Utc>) -> String {
    let local = at.with_timezone(&Local);
    let mins = (at - Utc::now()).num_minutes().max(0);
    let (h, m) = (mins / 60, mins % 60);
    if h < 24 {
        format!("en {h} h {m:02} min ({})", local.format("%H:%M"))
    } else {
        let day = ["lun", "mar", "mié", "jue", "vie", "sáb", "dom"]
            [local.weekday().num_days_from_monday() as usize];
        format!(
            "en {} d {} h ({day} {})",
            h / 24,
            h % 24,
            local.format("%H:%M")
        )
    }
}

impl Tray for UsageTray {
    const MENU_ON_ACTIVATE: bool = true;

    fn id(&self) -> String {
        "claude-usage-indicator".into()
    }

    fn title(&self) -> String {
        "Uso de Claude".into()
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        let u = self.usage();
        icon::render(
            u.and_then(|u| u.session.as_ref()).map(|l| l.percent),
            u.and_then(|u| u.weekly.as_ref()).map(|l| l.percent),
        )
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            title: "Uso de Claude".into(),
            description: self
                .summary_lines()
                .into_iter()
                .map(|(line, _)| line)
                .collect::<Vec<_>>()
                .join("\n"),
            ..Default::default()
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        // Habilitadas a propósito: GNOME atenúa las deshabilitadas y se leen mal.
        // Al pulsarlas no hacen nada (el `activate` por defecto está vacío).
        let info = |(label, percent): (String, Option<f64>)| {
            StandardItem {
                label: label.replace('_', "__"),
                icon_data: percent.map(icon::menu_png).unwrap_or_default(),
                ..Default::default()
            }
            .into()
        };

        let mut items: Vec<MenuItem<Self>> = Vec::new();
        if let Some(plan) = self.usage().and_then(|u| u.plan.as_ref()) {
            items.push(info((format!("Plan: {plan}"), None)));
        }
        items.extend(self.summary_lines().into_iter().map(info));
        if let State::Ok(_, at) = &self.state {
            items.push(info((
                format!("Actualizado a las {}", at.format("%H:%M:%S")),
                None,
            )));
        }
        items.push(MenuItem::Separator);
        items.push(
            StandardItem {
                label: "Actualizar ahora".into(),
                icon_name: "view-refresh".into(),
                activate: Box::new(|t: &mut Self| {
                    let _ = t.refresh.send(());
                }),
                ..Default::default()
            }
            .into(),
        );
        items.push(
            StandardItem {
                label: "Abrir página de uso".into(),
                icon_name: "web-browser".into(),
                activate: Box::new(|_| {
                    let _ = open::that_detached(USAGE_PAGE);
                }),
                ..Default::default()
            }
            .into(),
        );
        items.push(
            StandardItem {
                label: "Salir".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|_| std::process::exit(0)),
                ..Default::default()
            }
            .into(),
        );
        items
    }
}

fn interval() -> Duration {
    let secs = std::env::var("CLAUDE_USAGE_INTERVAL")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_INTERVAL_SECS)
        .max(60);
    Duration::from_secs(secs)
}

fn main() {
    let (tx, rx) = mpsc::channel();
    let tray = UsageTray {
        state: State::Loading,
        refresh: tx,
    };

    let handle = match tray.assume_sni_available(true).spawn() {
        Ok(h) => h,
        Err(e) => {
            eprintln!(
                "No se pudo crear el icono de bandeja: {e}\n\
                 En GNOME necesitas la extensión AppIndicator (ubuntu-appindicators o appindicatorsupport)."
            );
            std::process::exit(1);
        }
    };

    let interval = interval();
    loop {
        let result = api::fetch();
        if let Err(e) = &result {
            eprintln!("{e}");
        }
        handle.update(|t| {
            t.state = match result {
                Ok(u) => State::Ok(u, Local::now()),
                // Mantiene los últimos datos buenos para que el icono no se quede vacío.
                Err(e) => State::Error(e, t.usage().cloned()),
            };
        });

        match rx.recv_timeout(interval) {
            Ok(()) | Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
}
