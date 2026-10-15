# Lessons

Each folder here is one lesson that KoperCAD runs inside the program. The format, the checks and the
rules for good lessons are in [`docs/design/learning-modules.md`](../../docs/design/learning-modules.md).
This page is the short version for someone adding a lesson.

```
assets/lessons/
  tracks.toml        the tracks, in Learn-window order (framework owns)
  glossary.toml      words for {tip:term}; add a [term.<id>] table, never edit another thread's
  <lesson-id>/
    lesson.toml      the steps
    project.json     the sample project the lesson opens (or project = "@example" / "@blank")
    finished.json    optional: the same drawing at the end ("Open finished version")
```

## Adding a lesson

1. Make the folder `assets/lessons/<lesson-id>/` (lowercase letters, digits, hyphens).
2. Write `lesson.toml` with `status = "draft"`, a `track` from `tracks.toml`, and `order` in steps of 10.
3. Make the sample project. Build it in the app (or with a test builder), save it, then write its
   diffable form:

   ```
   kopercad-cli json my-project.kopercad > assets/lessons/<lesson-id>/project.json
   ```

   Keep it under 200 KB. **If `cargo test` says a `project.json` does not load, the project JSON
   format moved; run the command above again on the lesson's source project.** Nothing else in the
   lesson changes.
4. `cargo test -p novo-learn` parses every lesson and resolves every file it names. The checks that
   need the running program's commands and anchors run in `novo-app`.
5. Flip `status` to `ready` in the PR that lands the control the lesson teaches.

## Rules that save a review

- Teach the key first: write `{key:command.id}`, never the key itself.
- One action per step, three lines of text at most.
- Every string a person reads belongs in the file, so it can get a `_nl` twin (`title_nl`, `text_nl`).
- A step you want checked needs an `expect`; a step with none is a reading step with a Next button.
- Give every action step a `solve`, so the replay test and "Show me how" can run it.
