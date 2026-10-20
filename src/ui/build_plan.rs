use std::rc::Rc;

use gtk::prelude::*;

use crate::build_plan::BuildPlan;

use super::navigation::{back_button, connect_plan_button, connect_report_button};
use super::state::{AppState, ReportSource, ViewState};
use super::widgets::{clear_view, header_icon_button, label, message_label, report_key_hints};
use super::{
    REPORT_MIN_WINDOW_HEIGHT, REPORT_MIN_WINDOW_WIDTH, REPORT_WINDOW_HEIGHT, REPORT_WINDOW_WIDTH,
};

pub(super) fn show_build_plan(
    window: &gtk::ApplicationWindow,
    root: &gtk::Box,
    state: Rc<AppState>,
    source: ReportSource,
    flake: &str,
    plan: BuildPlan,
) {
    window.set_default_size(REPORT_WINDOW_WIDTH, REPORT_WINDOW_HEIGHT);
    window.set_size_request(REPORT_MIN_WINDOW_WIDTH, REPORT_MIN_WINDOW_HEIGHT);
    state.set_view(ViewState::BuildPlan);
    clear_view(window, root);
    root.remove_css_class("chooser-root");
    root.remove_css_class("message-root");

    let header = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    header.add_css_class("report-header");
    let heading = gtk::Box::new(gtk::Orientation::Vertical, 6);
    heading.set_hexpand(true);
    heading.append(&label("Build plan", &["title"], 0.0));
    heading.append(&label(
        &format!("{} · #{flake}", source.label()),
        &["subtitle"],
        0.0,
    ));
    header.append(&heading);

    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.set_valign(gtk::Align::Center);
    let report = gtk::Button::with_label("Full report");
    report.add_css_class("plan-button");
    report.set_tooltip_text(Some(
        "Build the generation and compare package changes (uses a cached report if available)",
    ));
    connect_report_button(&report, window, root, Rc::clone(&state), source);
    actions.append(&report);
    let refresh = header_icon_button("view-refresh-symbolic", "Refresh build plan");
    connect_plan_button(&refresh, window, root, Rc::clone(&state), source);
    actions.append(&refresh);
    actions.append(&back_button(window, root, Rc::clone(&state)));
    header.append(&actions);
    root.append(&header);

    let note = message_label(
        "Preview only · Shows builds and downloads needed, including build dependencies. Already available paths are omitted.",
        &["plan-note"],
        0.0,
    );
    root.append(&note);

    let summary = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    summary.add_css_class("plan-summary");
    summary.set_homogeneous(true);
    summary.append(&summary_card(plan.builds.len(), "To build", "plan-build"));
    summary.append(&summary_card(
        plan.downloads.len(),
        "To download",
        "plan-download",
    ));
    if !plan.unknown.is_empty() {
        summary.append(&summary_card(
            plan.unknown.len(),
            "Unavailable",
            "plan-unknown",
        ));
    }
    root.append(&summary);
    if let Some(sizes) = &plan.download_summary {
        root.append(&label(sizes, &["plan-estimate"], 0.0));
    }

    let content = gtk::Box::new(gtk::Orientation::Vertical, 20);
    content.add_css_class("plan-content");
    if plan.builds.is_empty() && plan.downloads.is_empty() && plan.unknown.is_empty() {
        let empty = gtk::Box::new(gtk::Orientation::Vertical, 10);
        empty.add_css_class("plan-empty");
        empty.append(&label(
            "No pending builds or downloads reported",
            &["no-changes"],
            0.5,
        ));
        empty.append(&message_label(
            if plan.output.is_empty() {
                "Available outputs can be reused. Open the full report to compare generations."
            } else {
                "See Nix output below for details. Open the full report to compare generations."
            },
            &["plan-note"],
            0.5,
        ));
        content.append(&empty);
    }
    for (title, paths, class) in [
        ("To build", &plan.builds, "plan-build"),
        ("To download", &plan.downloads, "plan-download"),
        ("Unavailable paths", &plan.unknown, "plan-unknown"),
    ] {
        if !paths.is_empty() {
            content.append(&path_section(title, paths, class));
        }
    }
    if !plan.output.is_empty() {
        let details = gtk::Expander::new(Some("Nix output"));
        details.add_css_class("plan-details");
        let output = message_label(&plan.output, &["plan-output"], 0.0);
        details.set_child(Some(&output));
        content.append(&details);
    }
    let scroller = gtk::ScrolledWindow::new();
    scroller.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroller.set_vexpand(true);
    scroller.set_child(Some(&content));
    state.register_scroll_adjustment(&scroller);
    root.append(&scroller);
    root.append(&report_key_hints(state.show_demo_enabled(), false));
}

fn summary_card(count: usize, title: &str, class: &str) -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 4);
    card.add_css_class("plan-card");
    card.add_css_class(class);
    card.append(&label(&count.to_string(), &["plan-count"], 0.0));
    card.append(&label(title, &["plan-card-label"], 0.0));
    card
}

fn path_section(title: &str, paths: &[String], class: &str) -> gtk::Box {
    let section = gtk::Box::new(gtk::Orientation::Vertical, 10);
    let heading = label(
        &format!("{title} · {}", paths.len()),
        &["plan-section-title", class],
        0.0,
    );
    section.append(&heading);
    let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
    list.add_css_class("plan-path-list");
    for path in paths {
        let row = gtk::Box::new(gtk::Orientation::Vertical, 4);
        row.add_css_class("plan-path-row");
        let name = path.rsplit('/').next().unwrap_or(path);
        let name = name.split_once('-').map(|(_, name)| name).unwrap_or(name);
        let name = name.strip_suffix(".drv").unwrap_or(name);
        let title = label(name, &["plan-package-name"], 0.0);
        title.set_single_line_mode(true);
        title.set_max_width_chars(70);
        row.append(&title);
        let location = label(path, &["plan-package-path"], 0.0);
        location.set_single_line_mode(true);
        location.set_max_width_chars(90);
        location.set_selectable(true);
        location.set_tooltip_text(Some(path));
        row.append(&location);
        list.append(&row);
    }
    section.append(&list);
    section
}
