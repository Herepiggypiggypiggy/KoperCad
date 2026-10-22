# Workflow: commissioning and handover

## Commissioning

**Ctrl+7** opens the Commissioning workspace: checks you do at the machine, ticked one by one with a name and time.

- The checks are listed per device and circuit of the design and stay in step with it as the design changes.
- A failed check becomes a **punch list** item with a link to the device or wire; so does a **Problem** raised in Build mode.
- Photos can be attached to a check.
- Your name is set once in this workspace; it is stamped on every tick. "(no name set)" appears if you did not set one.

### Field mode

Carry the project to a tablet, tick checks at the machine, then export a `.kcfield` file and import it back at the desk. Per item the newer decision wins; a tick for another project or a deleted device is skipped. Large buttons switch on by themselves in a narrow window.

## Revisions

File > **Revisions** issues a revision (a snapshot with a letter, date and note), compares two revisions, and locks pages that are finished.

- **Compare** shows what was added, removed or changed between two revisions, drawn on the page.
- A locked page cannot be edited until unlocked.
- **Restore page** brings a page back from a revision.

## The final set

Before handing over:

1. Open the Issues list and resolve or waive every problem. A waiver keeps its reason.
2. Run the Standards check (Check menu) and export the report (File > Export). Read [what it is and is not](numbers.md#standards-checks): a pass is not a certification.
3. Issue a revision.
4. Export the PDF of the whole set, the parts list and the wire list.
5. Add the commissioning results and punch list. File > Export > **Export handover package (folder)** gathers the set in one folder.

Everything is in the one project file, so copying the file hands over the whole project.
