# Commissioning column: handoff

- `novo-theme::widgets::progress_bar` (caption, counts, done in green, failed in red) is new and shared.
- `app/commissioning.rs`: header = `widgets::tabs` + one "By <name>" menu (name field, PDF and CSV report, Large buttons); list grouped by page with fold state in `open_pages`, rows from `widgets::row` (control height + 2; 36 pt controls only with Large on); check state icons from `state_icon`.
- Anchors `commissioning.who` and `commissioning.report` sit on the menu button while it is closed; `commissioning.punch` on the right half of the tab row. The Dutch lesson text now points at the By menu.
- Pictures: `/mnt/project-files/design/commissioning-column/`.
